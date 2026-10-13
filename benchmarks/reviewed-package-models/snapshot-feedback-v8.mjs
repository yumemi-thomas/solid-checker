// Follow one exact local return/call edge; never guess an escaped helper's caller.
import { existsSync, readFileSync } from 'node:fs';
import { ts } from './lower.mjs';
import { hash } from './catalog.mjs';
import { instrumentGuards } from './guard-trace.mjs';
import { classSnapshotFlowsV4, getterSnapshotsV7, snapshotFeedbackV7 } from './snapshot-feedback-v7.mjs';
export { classSnapshotFlowsV4 };
export const snapshotFeedbackV8 = snapshotFeedbackV7;
const unwrap = node => {
  while (node && (ts.isParenthesizedExpression(node) || ts.isAsExpression(node) || ts.isNonNullExpression(node) || ts.isSatisfiesExpression(node))) node = node.expression;
  return node;
};
const cache = new Map();
function authenticate(event) {
  if (event.kind !== 'tracking-skipped' || !existsSync(event.path)) return false;
  const text = readFileSync(event.path, 'utf8'), digest = hash(text); if (digest !== event.sourceSha256) return false;
  const key = event.path + ':' + digest;
  if (!cache.has(key)) cache.set(key, instrumentGuards(text, event.path)?.observations ?? []);
  return cache.get(key).some(premise => premise.kind === event.kind && premise.start === event.start && premise.end === event.end);
}
function setup(node) {
  const statement = node.parent?.parent, block = statement?.parent, owner = block?.parent;
  if (!ts.isVariableStatement(statement) || !ts.isBlock(block) || !ts.isFunctionLike(owner) || owner.body !== block) return false;
  let callback = owner;
  while (callback.parent && unwrap(callback.parent) === callback) callback = callback.parent;
  return !(ts.isCallExpression(callback.parent) && callback.parent.arguments.includes(callback));
}
function offset(source, location) {
  if (location?.path !== source.fileName || !Number.isInteger(location.line) || !Number.isInteger(location.column)) return null;
  const starts = source.getLineStarts(), line = location.line - 1, character = location.column - 1;
  if (line < 0 || line >= starts.length || character < 0) return null;
  const value = starts[line] + character;
  return value < (starts[line + 1] ?? source.end) ? value : null;
}
export function getterSnapshotsV8(program, source, events) {
  const result = getterSnapshotsV7(program, source, events), checker = program.getTypeChecker(), helpers = new Map();
  const collect = node => {
    let name, fn;
    if (ts.isFunctionDeclaration(node) && node.name) { name = node.name; fn = node; }
    else if (ts.isVariableDeclaration(node) && ts.isIdentifier(node.name) && node.initializer && node.parent.flags & ts.NodeFlags.Const) {
      const value = unwrap(node.initializer); if (ts.isArrowFunction(value) || ts.isFunctionExpression(value)) { name = node.name; fn = value; }
    }
    if (fn && fn.parameters.length === 0 && !fn.asteriskToken && !fn.modifiers?.some(modifier => modifier.kind === ts.SyntaxKind.AsyncKeyword)) {
      const body = fn.body, returned = body && (ts.isBlock(body)
        ? body.statements.length === 1 && ts.isReturnStatement(body.statements[0]) && body.statements[0].expression : body), expression = returned && unwrap(returned);
      if (expression && (ts.isPropertyAccessExpression(expression) || ts.isElementAccessExpression(expression))) {
        const symbol = checker.getSymbolAtLocation(name); if (symbol) helpers.set(symbol, { name, fn, returned, references: [] });
      }
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
  const openHelpers = [];
  for (const helper of helpers.values()) {
    if (helper.references.length !== 1) { openHelpers.push({ start: helper.name.getStart(source), reason: 'helper caller is not unique or helper escapes' }); continue; }
    let callee = helper.references[0]; while (callee.parent && unwrap(callee.parent) === callee) callee = callee.parent;
    const call = callee.parent, declaration = call?.parent;
    if (!ts.isCallExpression(call) || call.expression !== callee || call.questionDotToken || call.arguments.length ||
      !ts.isVariableDeclaration(declaration) || declaration.initializer !== call || !ts.isIdentifier(declaration.name) ||
      !(declaration.parent.flags & ts.NodeFlags.Const) || !setup(declaration)) continue;
    const symbol = checker.getSymbolAtLocation(declaration.name), jsxUses = [];
    const scan = (node, jsx = false) => {
      if (jsx && ts.isFunctionLike(node)) return;
      if (ts.isJsxExpression(node)) jsx = true;
      if (jsx && ts.isIdentifier(node) && checker.getSymbolAtLocation(node) === symbol) jsxUses.push({ start: node.getStart(source), end: node.end });
      ts.forEachChild(node, child => scan(child, jsx));
    }; scan(source); if (!jsxUses.length) continue;
    const returnedStart = helper.returned.getStart(source), returnedEnd = helper.returned.end, matching = (events ?? []).filter(event => {
      const position = offset(source, event.originalLocation); return position !== null && position >= returnedStart && position < returnedEnd && authenticate(event);
    });
    if (!matching.length) continue;
    const event = matching[0], start = call.getStart(source), end = call.end;
    if (!result.candidates.some(candidate => candidate.start === start && candidate.end === end)) result.candidates.push({
      code: 'OBSERVED_PACKAGE_SNAPSHOT_FLOW', severity: 'info', certification: false, category: 'intent-open', start, end, jsxUses,
      premise: { path: event.path, start: event.start, end: event.end, sourceSha256: event.sourceSha256 }, staticDispatch: 'open',
      localReturn: { path: source.fileName, helperStart: helper.name.getStart(source), returnStart: returnedStart, returnEnd: returnedEnd,
        callStart: start, callEnd: end, sourceSha256: hash(source.text), callerReferences: 1 },
      basis: 'authenticated guard inside one local property return with one exact setup call and a later JSX use',
      message: 'This displayed value was captured through a local helper while its package read skipped tracking. Call the helper inside JSX if the value should stay live; use untrack for an intentional snapshot.' });
  }
  return { ...result, open: [...result.open, ...openHelpers] };
}
