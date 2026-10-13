// Broaden consumer shapes while preserving explicit callback and intent gaps.
import { ts } from './lower.mjs';
import { existsSync, readFileSync } from 'node:fs';
import { hash } from './catalog.mjs';
import { instrumentGuards } from './guard-trace.mjs';
import { authenticatedGetterSnapshots, classSnapshotFlowsV2, snapshotFeedbackV3 } from './snapshot-feedback-v3.mjs';
const unwrap = node => {
  while (node && (ts.isParenthesizedExpression(node) || ts.isAsExpression(node) || ts.isNonNullExpression(node) || ts.isSatisfiesExpression(node))) node = node.expression;
  return node;
};
const guardCache = new Map();
function guardPremise(event) {
  if (!existsSync(event.path)) return null;
  const text = readFileSync(event.path, 'utf8'), digest = hash(text);
  if (event.sourceSha256 !== digest) return null;
  const key = event.path + ':' + digest;
  if (!guardCache.has(key)) guardCache.set(key, instrumentGuards(text, event.path)?.observations ?? []);
  return guardCache.get(key).find(premise => premise.kind === event.kind && premise.start === event.start && premise.end === event.end);
}
function enclosingSetup(source, start) {
  let declaration = null;
  const find = node => { if (ts.isVariableDeclaration(node) && node.initializer && node.initializer.getStart(source) === start) declaration = node;
    if (!declaration) ts.forEachChild(node, find); };
  find(source); const block = declaration?.parent?.parent?.parent, owner = block?.parent;
  if (!owner || !ts.isFunctionLike(owner)) return null;
  let callback = owner;
  while (callback.parent && unwrap(callback.parent) === callback) callback = callback.parent;
  return { declaration, owner, callbackArgument: ts.isCallExpression(callback.parent) && callback.parent.arguments.includes(callback) };
}
export function classSnapshotFlowsV4(program, source, engine) {
  const result = classSnapshotFlowsV2(program, source, engine), candidates = [];
  for (const candidate of result.candidates) {
    const context = enclosingSetup(source, candidate.start);
    if (context?.callbackArgument) result.refused.push({ start: candidate.start, reason: 'callback setup has no admitted snapshot phase' });
    else candidates.push(candidate);
  }
  return { ...result, candidates };
}
export function getterSnapshotsV4(program, source, events) {
  const result = authenticatedGetterSnapshots(program, source, events), checker = program.getTypeChecker();
  const visit = node => {
    if (ts.isVariableDeclaration(node) && ts.isObjectBindingPattern(node.name) && node.initializer && node.parent.flags & ts.NodeFlags.Const &&
      node.name.elements.length === 1 && !node.name.elements[0].dotDotDotToken && !node.name.elements[0].initializer &&
      ts.isIdentifier(node.name.elements[0].name) && !node.name.elements[0].propertyName?.expression) {
      const binding = node.name.elements[0], name = binding.name, context = enclosingSetup(source, node.initializer.getStart(source));
      if (context && !context.callbackArgument) {
        const symbol = checker.getSymbolAtLocation(name), uses = [];
        const scan = (child, jsx = false) => {
          if (jsx && ts.isFunctionLike(child)) return;
          if (ts.isJsxExpression(child)) jsx = true;
          if (jsx && ts.isIdentifier(child) && checker.getSymbolAtLocation(child) === symbol) uses.push({ start: child.getStart(source), end: child.end });
          ts.forEachChild(child, next => scan(next, jsx));
        }; scan(source);
        if (uses.length) {
          const rawStart = node.getStart(source), rawEnd = node.end;
          for (const event of events ?? []) {
            if (event.originalLocation?.path !== source.fileName) continue;
            const offset = source.getPositionOfLineAndCharacter(event.originalLocation.line - 1, event.originalLocation.column - 1);
            if (offset < rawStart || offset >= rawEnd || event.kind !== 'tracking-skipped') continue;
            const premise = guardPremise(event);
            if (premise) result.candidates.push({ code: 'OBSERVED_GETTER_SNAPSHOT_FLOW', severity: 'info', certification: false,
              category: 'intent-open', start: rawStart, end: rawEnd,
              premise: { path: event.path, start: event.start, end: event.end, sourceSha256: event.sourceSha256 },
              jsxUses: uses, binding: (binding.propertyName ?? name).text,
              basis: 'authenticated tracking guard during a single-field setup destructure used in JSX; liveness intent undeclared',
              message: 'This displayed value was destructured once while the package skipped tracking. Read the property inside JSX if it should stay live; use untrack for an intentional snapshot.' });
            else result.open.push({ start: rawStart, reason: 'destructuring guard premise is unavailable or changed' });
          }
        }
      }
    }
    ts.forEachChild(node, visit);
  }; visit(source); return result;
}
export const snapshotFeedbackV4 = snapshotFeedbackV3;
