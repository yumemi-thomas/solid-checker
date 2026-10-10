// Join a returned package read to its observed exact consumer call site.
// Additional references or escapes never authorize guessed member dispatch.
import { existsSync, readFileSync } from 'node:fs';
import { ts } from './lower.mjs';
import { hash } from './catalog.mjs';
import { instrumentGuards } from './guard-trace.mjs';
import { classSnapshotFlowsV4, getterSnapshotsV9, snapshotFeedbackV9 } from './snapshot-feedback-v9.mjs';
export { classSnapshotFlowsV4 };
export const snapshotFeedbackV10 = snapshotFeedbackV9;
const unwrap = node => {
  while (node && (ts.isParenthesizedExpression(node) || ts.isAsExpression(node) || ts.isNonNullExpression(node) || ts.isSatisfiesExpression(node))) node = node.expression;
  return node;
};
const guardCache = new Map();
function authenticate(event) {
  if (event.kind !== 'tracking-skipped' || !existsSync(event.path)) return false;
  const text = readFileSync(event.path, 'utf8'), digest = hash(text);
  if (digest !== event.sourceSha256) return false;
  const key = event.path + ':' + digest;
  if (!guardCache.has(key)) guardCache.set(key, instrumentGuards(text, event.path)?.observations ?? []);
  return guardCache.get(key).some(premise => premise.kind === event.kind && premise.start === event.start && premise.end === event.end);
}
function offset(source, frame) {
  if (frame?.path !== source.fileName || frame.sourceSha256 !== hash(source.text) ||
    !Number.isInteger(frame.line) || !Number.isInteger(frame.column)) return null;
  const starts = source.getLineStarts(), line = frame.line - 1, character = frame.column - 1;
  if (line < 0 || line >= starts.length || character < 0) return null;
  const value = starts[line] + character;
  return value < (starts[line + 1] ?? source.end) ? value : null;
}
function setup(declaration) {
  const statement = declaration.parent?.parent, block = statement?.parent, owner = block?.parent;
  if (!ts.isVariableStatement(statement) || !ts.isBlock(block) || !ts.isFunctionLike(owner) || owner.body !== block) return false;
  let callback = owner;
  while (callback.parent && unwrap(callback.parent) === callback) callback = callback.parent;
  return !(ts.isCallExpression(callback.parent) && callback.parent.arguments.includes(callback));
}
function isWrite(reference) {
  for (let node = reference; node.parent && !ts.isFunctionLike(node.parent); node = node.parent) {
    const parent = node.parent;
    if (ts.isBinaryExpression(parent) && parent.left === node && parent.operatorToken.kind >= ts.SyntaxKind.FirstAssignment && parent.operatorToken.kind <= ts.SyntaxKind.LastAssignment ||
      (ts.isPrefixUnaryExpression(parent) || ts.isPostfixUnaryExpression(parent)) &&
      (parent.operator === ts.SyntaxKind.PlusPlusToken || parent.operator === ts.SyntaxKind.MinusMinusToken) ||
      (ts.isForInStatement(parent) || ts.isForOfStatement(parent)) && parent.initializer === node) return true;
    if (ts.isStatement(parent)) break;
  }
  return false;
}
export function getterSnapshotsV10(program, source, events) {
  const result = getterSnapshotsV9(program, source, events), checker = program.getTypeChecker(), helpers = new Map();
  // Legacy observations retain the earlier unique-reference inference. Once
  // caller frames exist, local helper hints must carry the observed call edge.
  const hasCallerProfile = (events ?? []).some(event => Array.isArray(event.originalFrames));
  const candidates = result.candidates.filter(candidate => !hasCallerProfile || !candidate.localReturn);
  const collect = node => {
    let name, fn;
    if (ts.isFunctionDeclaration(node) && node.name) { name = node.name; fn = node; }
    else if (ts.isVariableDeclaration(node) && ts.isIdentifier(node.name) && node.initializer && node.parent.flags & ts.NodeFlags.Const) {
      const value = unwrap(node.initializer);
      if (ts.isArrowFunction(value) || ts.isFunctionExpression(value)) { name = node.name; fn = value; }
    }
    if (fn && fn.parameters.length === 0 && !fn.asteriskToken && !fn.modifiers?.some(modifier => modifier.kind === ts.SyntaxKind.AsyncKeyword)) {
      const body = fn.body, returned = body && (ts.isBlock(body)
        ? body.statements.length === 1 && ts.isReturnStatement(body.statements[0]) && body.statements[0].expression : body), expression = returned && unwrap(returned);
      let unsupported = !expression || !(ts.isPropertyAccessExpression(expression) || ts.isElementAccessExpression(expression));
      const check = child => {
        if (ts.isCallExpression(child) || ts.isNewExpression(child) || ts.isFunctionLike(child)) unsupported = true;
        ts.forEachChild(child, check);
      };
      if (expression) check(expression);
      const symbol = checker.getSymbolAtLocation(name);
      if (!unsupported && symbol?.declarations?.length === 1) helpers.set(symbol, { name, fn, returned, references: [] });
    }
    ts.forEachChild(node, collect);
  }; collect(source);
  const references = node => {
    if (ts.isTypeNode(node) || ts.isImportDeclaration(node)) return;
    if (ts.isIdentifier(node)) {
      const helper = helpers.get(checker.getSymbolAtLocation(node));
      if (helper && node !== helper.name) helper.references.push(node);
    }
    ts.forEachChild(node, references);
  }; references(source);
  for (const helper of helpers.values()) {
    if (helper.references.some(isWrite)) { result.open.push({ start: helper.name.getStart(source), reason: 'helper binding is assigned' }); continue; }
    for (const reference of helper.references) {
      let callee = reference;
      while (callee.parent && unwrap(callee.parent) === callee) callee = callee.parent;
      const call = callee.parent;
      if (!ts.isCallExpression(call) || call.expression !== callee || call.arguments.length || call.questionDotToken) continue;
      let initializer = call;
      while (initializer.parent && unwrap(initializer.parent) === initializer) initializer = initializer.parent;
      const declaration = initializer.parent;
      if (!ts.isVariableDeclaration(declaration) || declaration.initializer !== initializer || !ts.isIdentifier(declaration.name) ||
        !(declaration.parent.flags & ts.NodeFlags.Const) || !setup(declaration)) continue;
      const symbol = checker.getSymbolAtLocation(declaration.name), jsxUses = [];
      const scan = (node, jsx = false) => {
        if (jsx && ts.isFunctionLike(node)) return;
        if (ts.isJsxExpression(node)) jsx = true;
        if (jsx && ts.isIdentifier(node) && checker.getSymbolAtLocation(node) === symbol) jsxUses.push({ start: node.getStart(source), end: node.end });
        ts.forEachChild(node, child => scan(child, jsx));
      }; scan(source);
      if (!jsxUses.length) continue;
      const returnStart = helper.returned.getStart(source), returnEnd = helper.returned.end, start = call.getStart(source), end = call.end;
      let witness;
      for (const event of events ?? []) {
        if (!Array.isArray(event.originalFrames) || !authenticate(event)) continue;
        const frames = event.originalFrames.map((frame, index) => ({ frame, index })).filter(({ frame }) => frame?.path === source.fileName);
        const returnedPosition = offset(source, frames[0]?.frame), callPosition = offset(source, frames[1]?.frame);
        if (returnedPosition === null || returnedPosition < returnStart || returnedPosition >= returnEnd ||
          callPosition === null || callPosition < start || callPosition >= end) continue;
        witness = { event, frames: frames.slice(0, 2) }; break;
      }
      if (!witness || candidates.some(candidate => candidate.start === start && candidate.end === end)) continue;
      const { event, frames } = witness;
      candidates.push({ code: 'OBSERVED_PACKAGE_SNAPSHOT_FLOW', severity: 'info', certification: false, category: 'intent-open',
        start, end, jsxUses, staticDispatch: 'open',
        premise: { path: event.path, start: event.start, end: event.end, sourceSha256: event.sourceSha256 },
        localReturn: { path: source.fileName, helperStart: helper.name.getStart(source), returnStart, returnEnd,
          callStart: start, callEnd: end, sourceSha256: hash(source.text), callerReferences: helper.references.length },
        observedCall: { returnFrame: frames[0].frame, callFrame: frames[1].frame,
          returnFrameIndex: frames[0].index, callFrameIndex: frames[1].index },
        basis: 'authenticated package guard and source-mapped return/caller frames at an exact local setup call with a later JSX use',
        message: 'This displayed value was captured through a local helper while its package read skipped tracking. Call the helper inside JSX if the value should stay live; use untrack for an intentional snapshot.' });
    }
  }
  return { ...result, candidates };
}
