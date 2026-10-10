// Observe retained initializer reads without guessing a package call target.
// Unexecuted paths and liveness intent remain open.
import { ts } from './lower.mjs';
import { existsSync, readFileSync } from 'node:fs';
import { hash } from './catalog.mjs';
import { instrumentGuards } from './guard-trace.mjs';
import { classSnapshotFlowsV4, getterSnapshotsV5, snapshotFeedbackV5 } from './snapshot-feedback-v5.mjs';
export { classSnapshotFlowsV4 };
export const snapshotFeedbackV6 = snapshotFeedbackV5;
const unwrap = node => {
  while (node && (ts.isParenthesizedExpression(node) || ts.isAsExpression(node) || ts.isNonNullExpression(node) || ts.isSatisfiesExpression(node))) node = node.expression;
  return node;
};
const inside = (source, node, offset) => node && offset >= node.getStart(source) && offset < node.end;
function locationOffset(source, location) {
  if (location?.path !== source.fileName || !Number.isInteger(location.line) || !Number.isInteger(location.column)) return null;
  const lines = source.getLineStarts(), line = location.line - 1, character = location.column - 1;
  if (line < 0 || line >= lines.length || character < 0) return null;
  const offset = lines[line] + character, limit = lines[line + 1] ?? source.end;
  return offset < limit ? offset : null;
}
function retained(source, expression, offset) {
  if (!inside(source, expression, offset) || ts.isFunctionLike(expression)) return false;
  const node = unwrap(expression);
  if (ts.isPropertyAccessExpression(node)) return inside(source, node.name, offset) || retained(source, node.expression, offset);
  if (ts.isElementAccessExpression(node)) return retained(source, node.expression, offset) || retained(source, node.argumentExpression, offset);
  if (ts.isIdentifier(node) || ts.isStringLiteral(node) || ts.isNumericLiteral(node)) return true;
  if (ts.isBinaryExpression(node)) {
    if (node.operatorToken.kind === ts.SyntaxKind.CommaToken) return retained(source, node.right, offset);
    if (node.operatorToken.kind >= ts.SyntaxKind.FirstAssignment && node.operatorToken.kind <= ts.SyntaxKind.LastAssignment) return false;
    return retained(source, node.left, offset) || retained(source, node.right, offset);
  }
  if (ts.isPrefixUnaryExpression(node)) return retained(source, node.operand, offset);
  if (ts.isConditionalExpression(node)) return [node.condition, node.whenTrue, node.whenFalse].some(child => retained(source, child, offset));
  if (ts.isArrayLiteralExpression(node)) return node.elements.some(child => !ts.isSpreadElement(child) && retained(source, child, offset));
  if (ts.isObjectLiteralExpression(node)) return node.properties.some(child => ts.isPropertyAssignment(child) &&
    !ts.isComputedPropertyName(child.name) && retained(source, child.initializer, offset));
  if (ts.isTemplateExpression(node)) return node.templateSpans.some(span => retained(source, span.expression, offset));
  // A recorded package guard at the invoked callee supports an observed hint.
  // Argument reads do not establish that an unknown helper retains their value.
  if (ts.isCallExpression(node)) return retained(source, node.expression, offset);
  return false;
}
function setup(node) {
  const statement = node.parent?.parent, block = statement?.parent, owner = block?.parent;
  if (!ts.isVariableStatement(statement) || !ts.isBlock(block) || !ts.isFunctionLike(owner) || owner.body !== block) return false;
  let callback = owner;
  while (callback.parent && unwrap(callback.parent) === callback) callback = callback.parent;
  return !(ts.isCallExpression(callback.parent) && callback.parent.arguments.includes(callback));
}
export function getterSnapshotsV6(program, source, events) {
  const checker = program.getTypeChecker(), nativeUntrack = new Set(), markers = [], declarations = [];
  const unalias = symbol => symbol?.flags & ts.SymbolFlags.Alias ? checker.getAliasedSymbol(symbol) : symbol;
  for (const node of source.statements) if (ts.isImportDeclaration(node) && node.moduleSpecifier.text === 'solid-js') {
    const module = checker.getSymbolAtLocation(node.moduleSpecifier);
    if (module) for (const symbol of checker.getExportsOfModule(module)) if (symbol.name === 'untrack') nativeUntrack.add(unalias(symbol));
  }
  function explicit(callee, seen = new Set()) {
    const node = unwrap(callee), symbol = unalias(checker.getSymbolAtLocation(ts.isPropertyAccessExpression(node) ? node.name : node));
    if (nativeUntrack.has(symbol)) return true;
    if (!symbol || seen.has(symbol)) return false; seen.add(symbol);
    const declaration = symbol.valueDeclaration;
    return ts.isIdentifier(node) && declaration && ts.isVariableDeclaration(declaration) &&
      declaration.parent.flags & ts.NodeFlags.Const && declaration.initializer ? explicit(declaration.initializer, seen) : false;
  }
  const collect = node => {
    if (ts.isCallExpression(node) && node.arguments[0] && explicit(node.expression)) markers.push(node.arguments[0]);
    if (ts.isVariableDeclaration(node) && node.initializer && node.parent.flags & ts.NodeFlags.Const && setup(node)) declarations.push(node);
    ts.forEachChild(node, collect);
  }; collect(source);
  const suppressedObservations = [], admitted = [];
  for (const event of events ?? []) {
    const offset = locationOffset(source, event.originalLocation);
    if (offset === null) { suppressedObservations.push({ reason: 'missing or invalid consumer location' }); continue; }
    if (markers.some(marker => inside(source, marker, offset))) {
      suppressedObservations.push({ offset, reason: 'exact native untrack argument' }); continue;
    }
    const declaration = declarations.find(node => ts.isIdentifier(node.name) && inside(source, node.initializer, offset));
    if (declaration && !retained(source, declaration.initializer, offset)) {
      suppressedObservations.push({ offset, reason: 'discarded, deferred or unknown argument value flow' }); continue;
    }
    admitted.push(event);
  }
  const result = getterSnapshotsV5(program, source, admitted), pending = [];
  for (const declaration of declarations.filter(node => ts.isIdentifier(node.name))) {
    const symbol = checker.getSymbolAtLocation(declaration.name), jsxUses = [];
    const scan = (node, jsx = false) => {
      if (jsx && ts.isFunctionLike(node)) return;
      if (ts.isJsxExpression(node)) jsx = true;
      if (jsx && ts.isIdentifier(node) && checker.getSymbolAtLocation(node) === symbol) jsxUses.push({ start: node.getStart(source), end: node.end });
      ts.forEachChild(node, child => scan(child, jsx));
    }; scan(source); if (!jsxUses.length) continue;
    const start = declaration.initializer.getStart(source), end = declaration.initializer.end;
    if (result.candidates.some(candidate => candidate.start >= start && candidate.end <= end)) continue;
    for (const event of admitted) {
      const offset = locationOffset(source, event.originalLocation);
      if (!retained(source, declaration.initializer, offset)) continue;
      pending.push({ event, start, end, jsxUses });
    }
  }
  // Authenticate directly; no virtual consumer, native analogue or inferred
  // dispatch is needed for the observed initializer claim.
  for (const item of pending) {
    if (!authenticGuard(item.event)) continue;
    if (result.candidates.some(candidate => candidate.start === item.start && candidate.end === item.end)) continue;
    result.candidates.push({ code: 'OBSERVED_PACKAGE_SNAPSHOT_FLOW', severity: 'info', certification: false,
      category: 'intent-open', start: item.start, end: item.end, jsxUses: item.jsxUses,
      premise: { path: item.event.path, start: item.event.start, end: item.event.end, sourceSha256: item.event.sourceSha256 },
      staticDispatch: 'open', basis: 'authenticated tracking-skip observation retained by a setup initializer used in JSX',
      message: 'A package read in this displayed value skipped tracking during setup. Move live reads into JSX; keep intentional snapshots in untrack.' });
  }
  return { ...result, suppressedObservations };
}
const guardCache = new Map();
function authenticGuard(event) {
  if (event.kind !== 'tracking-skipped' || !existsSync(event.path)) return false;
  const text = readFileSync(event.path, 'utf8'), digest = hash(text); if (event.sourceSha256 !== digest) return false;
  const key = event.path + ':' + digest;
  if (!guardCache.has(key)) guardCache.set(key, instrumentGuards(text, event.path)?.observations ?? []);
  return guardCache.get(key).some(item => item.kind === event.kind && item.start === event.start && item.end === event.end);
}
