// Bounded local return chains with exact symbols and actual caller frames.
import { existsSync, readFileSync } from 'node:fs';
import { ts } from './lower.mjs';
import { hash } from './catalog.mjs';
import { instrumentGuards } from './guard-trace.mjs';
import { classSnapshotFlowsV4, getterSnapshotsV10, snapshotFeedbackV10 } from './snapshot-feedback-v10.mjs';
export { classSnapshotFlowsV4 };
export const snapshotFeedbackV11 = snapshotFeedbackV10;
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
function setup(declaration) {
  const statement = declaration.parent?.parent, block = statement?.parent, owner = block?.parent;
  if (!ts.isVariableStatement(statement) || !ts.isBlock(block) || !ts.isFunctionLike(owner) || owner.body !== block) return false;
  let callback = owner;
  while (callback.parent && unwrap(callback.parent) === unwrap(callback)) callback = callback.parent;
  return !(ts.isCallExpression(callback.parent) && callback.parent.arguments.includes(callback));
}
export function getterSnapshotsV11(program, source, events) {
  const result = getterSnapshotsV10(program, source, events), checker = program.getTypeChecker(), helpers = new Map(), calls = [], digest = hash(source.text);
  const collect = node => {
    let name, fn;
    if (ts.isFunctionDeclaration(node) && node.name) { name = node.name; fn = node; }
    else if (ts.isVariableDeclaration(node) && ts.isIdentifier(node.name) && node.initializer && node.parent.flags & ts.NodeFlags.Const) {
      const value = unwrap(node.initializer);
      if (ts.isArrowFunction(value) || ts.isFunctionExpression(value)) { name = node.name; fn = value; }
    }
    if (fn && !fn.asteriskToken && !fn.modifiers?.some(modifier => modifier.kind === ts.SyntaxKind.AsyncKeyword) &&
      fn.parameters.every(parameter => ts.isIdentifier(parameter.name) && !parameter.initializer && !parameter.dotDotDotToken)) {
      const body = fn.body, returned = body && (ts.isBlock(body)
        ? body.statements.length === 1 && ts.isReturnStatement(body.statements[0]) && body.statements[0].expression : body), expression = unwrap(returned),
        symbol = checker.getSymbolAtLocation(name);
      if (expression && symbol?.declarations?.length === 1) helpers.set(symbol, { name, fn, returned, expression });
    }
    if (ts.isCallExpression(node)) calls.push(node);
    ts.forEachChild(node, collect);
  }; collect(source);
  function target(expression, seen = new Set(), aliases = []) {
    expression = unwrap(expression); if (!expression || !ts.isIdentifier(expression)) return null;
    const symbol = checker.getSymbolAtLocation(expression);
    if (!symbol || seen.has(symbol)) return null;
    seen.add(symbol);
    if (helpers.has(symbol)) return { helper: helpers.get(symbol), aliases };
    const declarations = symbol.declarations;
    if (declarations?.length !== 1) return null;
    const declaration = declarations[0];
    if (!ts.isVariableDeclaration(declaration) || declaration.getSourceFile() !== source || !(declaration.parent.flags & ts.NodeFlags.Const) ||
      !ts.isIdentifier(declaration.name) || !declaration.initializer || !ts.isIdentifier(unwrap(declaration.initializer))) return null;
    return target(declaration.initializer, seen, [...aliases, { start: declaration.name.getStart(source), end: declaration.name.end }]);
  }
  function chain(resolved, seen = new Set()) {
    if (!resolved || seen.has(resolved.helper) || seen.size >= 4) return null;
    const helper = resolved.helper; seen.add(helper);
    if (ts.isCallExpression(helper.expression)) {
      if (helper.expression.questionDotToken) return null;
      const inner = chain(target(helper.expression.expression), seen);
      return inner && [...inner, { ...helper, aliases: resolved.aliases }];
    }
    if (!(ts.isPropertyAccessExpression(helper.expression) || ts.isElementAccessExpression(helper.expression))) return null;
    let unsupported = false;
    const scan = node => {
      if (ts.isCallExpression(node) || ts.isNewExpression(node) || ts.isFunctionLike(node) || ts.isAwaitExpression(node)) unsupported = true;
      ts.forEachChild(node, scan);
    }; scan(helper.expression);
    return unsupported ? null : [{ ...helper, aliases: resolved.aliases }];
  }
  function position(frame) {
    if (frame?.path !== source.fileName || frame.sourceSha256 !== digest || !Number.isInteger(frame.line) || !Number.isInteger(frame.column)) return null;
    const starts = source.getLineStarts(), line = frame.line - 1, character = frame.column - 1;
    if (line < 0 || line >= starts.length || character < 0) return null;
    const offset = starts[line] + character;
    return offset < (starts[line + 1] ?? source.end) ? offset : null;
  }
  function atCall(offset, call) {
    return offset !== null && offset >= call.getStart(source) && offset < call.end &&
      ![...call.arguments, ...call.typeArguments ?? []].some(argument => offset >= argument.getStart(source) && offset < argument.end);
  }
  for (const call of calls) {
    if (call.questionDotToken) continue;
    let initializer = call;
    while (initializer.parent && unwrap(initializer.parent) === unwrap(initializer)) initializer = initializer.parent;
    const declaration = initializer.parent;
    if (!ts.isVariableDeclaration(declaration) || declaration.initializer !== initializer || !ts.isIdentifier(declaration.name) ||
      !(declaration.parent.flags & ts.NodeFlags.Const) || !setup(declaration)) continue;
    const start = call.getStart(source), end = call.end;
    if (result.candidates.some(candidate => candidate.start === start && candidate.end === end)) continue;
    const path = chain(target(call.expression)); if (!path) continue;
    const captured = checker.getSymbolAtLocation(declaration.name), jsxUses = [];
    const scan = (node, jsx = false) => {
      if (jsx && ts.isFunctionLike(node)) return;
      if (ts.isJsxExpression(node)) jsx = true;
      if (jsx && ts.isIdentifier(node) && checker.getSymbolAtLocation(node) === captured) jsxUses.push({ start: node.getStart(source), end: node.end });
      ts.forEachChild(node, child => scan(child, jsx));
    }; scan(source);
    if (!jsxUses.length) continue;
    let witness;
    for (const event of events ?? []) {
      if (!Array.isArray(event.originalFrames) || !authenticate(event)) continue;
      const frames = event.originalFrames.map((frame, index) => ({ frame, index })).filter(({ frame }) => frame?.path === source.fileName);
      if (frames.length < path.length + 1) continue;
      if (path.some((helper, index) => {
        const offset = position(frames[index].frame);
        return ts.isCallExpression(helper.expression) ? !atCall(offset, helper.expression)
          : offset === null || offset < helper.returned.getStart(source) || offset >= helper.returned.end;
      })) continue;
      const callerPosition = position(frames[path.length].frame);
      if (!atCall(callerPosition, call)) continue;
      witness = { event, frames: frames.slice(0, path.length + 1) }; break;
    }
    if (!witness) continue;
    result.candidates.push({ code: 'OBSERVED_PACKAGE_SNAPSHOT_FLOW', severity: 'info', certification: false, category: 'intent-open',
      start, end, jsxUses, staticDispatch: 'open',
      premise: { path: witness.event.path, start: witness.event.start, end: witness.event.end, sourceSha256: witness.event.sourceSha256 },
      observedPath: { sourcePath: source.fileName, sourceSha256: digest, setupCall: { start, end }, frames: witness.frames,
        returns: path.map(helper => ({ helperStart: helper.name.getStart(source), returnStart: helper.returned.getStart(source), returnEnd: helper.returned.end,
          aliases: helper.aliases })) },
      basis: 'authenticated package guard and mapped frames across exact immutable aliases and at most four single-return local helpers',
      message: 'This displayed value was captured through local helpers while its package read skipped tracking. Read it inside JSX if it should stay live; use untrack for an intentional snapshot.' });
  }
  return result;
}
