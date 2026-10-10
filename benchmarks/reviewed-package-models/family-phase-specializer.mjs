// Combine independent return and callback-phase premises from exact installed
// source. This supplies a warning assumption, never package certification.
import { readFileSync } from 'node:fs';
import { hash } from './catalog.mjs';
import { argumentValue, demandSpecializer } from './demand-models.mjs';
import { CallbackPaths } from './callback-paths.mjs';
import { unknownValue } from './source-extractor.mjs';
import { ts } from './lower.mjs';
export function trackedCallbackPremise(profile, index) {
  const mandatory = profile.assumptions.filter(op => op.id === index);
  const observed = profile.footprints.filter(op => op.id === index);
  return !profile.gaps.length && mandatory.length > 0 && observed.length > 0 &&
    [...mandatory, ...observed].every(op => op.context === 'tracked-compute');
}
function unwrap(node) {
  while (ts.isParenthesizedExpression(node) || ts.isAsExpression(node) || ts.isTypeAssertionExpression(node) ||
    ts.isNonNullExpression(node) || ts.isSatisfiesExpression(node)) node = node.expression;
  return node;
}
export function phaseSpecializer(catalog, root) {
  const base = demandSpecializer(catalog, root), engines = new Map(), cache = new Map();
  const sourcePins = ['family-phase-specializer.mjs', 'callback-paths.mjs', 'source-extractor.mjs'].map(name => {
    const path = new URL(name, import.meta.url).pathname; return { path, sha256: hash(readFileSync(path)) };
  });
  const specialize = request => {
    const result = base(request), { node, model, host, name } = request;
    // The existing lowerer supports a single inline compute function and an
    // accessor return. Other protocols remain with their original premises.
    if (result.behavior.returns !== 'accessor' || node.arguments.length !== 1) return result;
    const argument = unwrap(node.arguments[0]);
    if (!ts.isArrowFunction(argument) && !ts.isFunctionExpression(argument)) return result;
    const args = node.arguments.map((node, index) => {
      const value = unwrap(node);
      return ts.isArrowFunction(value) || ts.isFunctionExpression(value) ?
        { kind: 'callback-probe', id: index, returnValue: unknownValue('consumer callback result is opaque') } : argumentValue(value);
    });
    const entry = model.runtimeEntries[host], key = JSON.stringify([model.pins, entry, host, name, args]);
    if (!cache.has(key)) {
      const engineKey = JSON.stringify([entry, host]);
      if (!engines.has(engineKey)) engines.set(engineKey, new CallbackPaths(host));
      cache.set(key, engines.get(engineKey).profile(entry, name, args));
    }
    const phase = cache.get(key), admitted = trackedCallbackPremise(phase, 0);
    return { ...result, behavior: { ...result.behavior, ...(admitted ? { trackedCallback: 0 } : {}) },
      callbackPhase: { ...phase, admitted, sourcePins } };
  };
  specialize.stats = base.stats;
  return specialize;
}
