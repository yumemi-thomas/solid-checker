// Execution can support a snapshot hint without proving computed dispatch.
// Static member dispatch, intended liveness and unexecuted paths stay open.
import { readFileSync, existsSync } from 'node:fs';
import { hash } from './catalog.mjs';
import { ts } from './lower.mjs';
import { instrumentGuards } from './guard-trace.mjs';
import { classSnapshotFlowsV4, getterSnapshotsV4, snapshotFeedbackV4 } from './snapshot-feedback-v4.mjs';
export { classSnapshotFlowsV4 };
export const snapshotFeedbackV5 = snapshotFeedbackV4;
const cache = new Map(), unwrap = node => {
  while (node && (ts.isParenthesizedExpression(node) || ts.isAsExpression(node) || ts.isNonNullExpression(node) || ts.isSatisfiesExpression(node))) node = node.expression;
  return node;
};
function authenticate(event) {
  if (event.kind !== 'tracking-skipped' || !existsSync(event.path)) return false;
  const text = readFileSync(event.path, 'utf8'), digest = hash(text); if (digest !== event.sourceSha256) return false;
  const key = event.path + ':' + digest;
  if (!cache.has(key)) cache.set(key, instrumentGuards(text, event.path)?.observations ?? []);
  return cache.get(key).some(item => item.kind === event.kind && item.start === event.start && item.end === event.end);
}
function setup(node) {
  const statement = node.parent?.parent, block = statement?.parent, owner = block?.parent;
  if (!ts.isVariableStatement(statement) || !ts.isBlock(block) || !ts.isFunctionLike(owner) || owner.body !== block) return false;
  let callback = owner;
  while (callback.parent && unwrap(callback.parent) === callback) callback = callback.parent;
  return !(ts.isCallExpression(callback.parent) && callback.parent.arguments.includes(callback));
}
export function getterSnapshotsV5(program, source, events) {
  const result = getterSnapshotsV4(program, source, events), checker = program.getTypeChecker();
  function uses(symbols) {
    const matches = [];
    const scan = (node, jsx = false) => {
      if (jsx && ts.isFunctionLike(node)) return;
      if (ts.isJsxExpression(node)) jsx = true;
      if (jsx && ts.isIdentifier(node) && symbols.includes(checker.getSymbolAtLocation(node))) matches.push({ start: node.getStart(source), end: node.end });
      ts.forEachChild(node, child => scan(child, jsx));
    }; scan(source); return matches;
  }
  const visit = node => {
    if (ts.isVariableDeclaration(node) && node.initializer && node.parent.flags & ts.NodeFlags.Const && setup(node)) {
      const expression = unwrap(node.initializer), computed = ts.isIdentifier(node.name) && ts.isCallExpression(expression) &&
        ts.isElementAccessExpression(unwrap(expression.expression)) && !expression.questionDotToken &&
        !unwrap(expression.expression).questionDotToken,
        multiple = ts.isObjectBindingPattern(node.name) && node.name.elements.length > 1 && node.name.elements.every(binding =>
          ts.isIdentifier(binding.name) && !binding.dotDotDotToken && !binding.initializer && !binding.propertyName?.expression);
      if (computed || multiple) {
        const symbols = (computed ? [node.name] : node.name.elements.map(binding => binding.name)).map(name => checker.getSymbolAtLocation(name));
        const jsxUses = symbols.every(Boolean) ? uses(symbols) : [];
        if (jsxUses.length) {
          const start = computed ? expression.getStart(source) : node.getStart(source), end = computed ? expression.end : node.end;
          const matching = (events ?? []).filter(event => {
            if (event.originalLocation?.path !== source.fileName || !authenticate(event)) return false;
            const location = event.originalLocation, offset = source.getPositionOfLineAndCharacter(location.line - 1, location.column - 1);
            return offset >= start && offset < end;
          });
          if (matching.length && !result.candidates.some(candidate => candidate.start >= start && candidate.end <= end)) {
            const event = matching[0];
            result.candidates.push({ code: 'OBSERVED_PACKAGE_SNAPSHOT_FLOW', severity: 'info', certification: false,
              category: 'intent-open', start, end, jsxUses, observedGuardCount: matching.length,
              premise: { path: event.path, start: event.start, end: event.end, sourceSha256: event.sourceSha256 },
              staticDispatch: 'open', basis: 'authenticated executed tracking guard within a setup expression whose value is used in JSX',
              message: 'This displayed value was computed once while a package skipped tracking. If it should stay live, read it inside JSX; use untrack for an intentional snapshot.' });
          }
        }
      }
    }
    ts.forEachChild(node, visit);
  }; visit(source); return result;
}
