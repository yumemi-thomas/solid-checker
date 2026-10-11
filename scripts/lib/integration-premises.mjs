// ADR 0271: repository-authored metadata, outside the public contract format.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';

export function validateIntegrationPremises(spec) {
  const premises = spec.premises ?? [];
  assert(Array.isArray(premises) && premises.length <= 1, 'one integration premise per authored spec');
  const keys = (value, allowed) => {
    assert(value && typeof value === 'object' && !Array.isArray(value), 'premise metadata must be an object');
    assert.deepEqual(Object.keys(value).sort(), allowed.sort(), 'unknown or missing premise metadata');
  };
  const identity = value => {
    keys(value, ['path', 'start', 'end']);
    assert(typeof value.path === 'string' && value.path.length > 0 && !value.path.startsWith('/') && !value.path.includes('\\') && !value.path.split('/').includes('..'), 'package-relative declaration path');
    assert(Number.isSafeInteger(value.start) && value.start >= 0 && Number.isSafeInteger(value.end) && value.end > value.start, 'exact declaration name-node span');
  };
  for (const premise of premises) {
    keys(premise, ['name', 'exports', 'valueReferences', 'configCalls']);
    assert(typeof premise.name === 'string' && premise.name.length > 0, 'named premise');
    assert(Array.isArray(premise.exports) && premise.exports.length > 0 && new Set(premise.exports).size === premise.exports.length, 'unique premised exports');
    for (const name of premise.exports) assert(spec.exports[name], `premise names absent export ${name}`);
    assert(Array.isArray(premise.valueReferences) && Array.isArray(premise.configCalls), 'premise veto catalog');
    assert(premise.valueReferences.length + premise.configCalls.length > 0, 'premise needs visible veto identities');
    premise.valueReferences.forEach(identity);
    for (const call of premise.configCalls) {
      keys(call, ['declaration', 'property', 'factories']);
      identity(call.declaration);
      assert(typeof call.property === 'string' && call.property.length > 0, 'config property');
      assert(Array.isArray(call.factories) && call.factories.length > 0, 'config factories');
      call.factories.forEach(identity);
    }
  }
}

export function integrationPremiseProbeDigest(spec, name, pair, misuse, correct, artifacts) {
  const premises = (spec.premises ?? []).filter(premise => premise.exports.includes(name));
  if (premises.length === 0) return undefined;
  return createHash('sha256').update(JSON.stringify({format:'integration-premise-probe-v1',
    premises, call:spec.exports[name].call, package:spec.package, version:spec.version,
    solidRuntime:spec.solidRuntime, pair, misuse, correct, artifacts})).digest('hex');
}
