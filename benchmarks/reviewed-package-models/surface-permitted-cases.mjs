import assert from 'node:assert/strict';
import cases from './surface-cases.mjs';
export default cases.map(entry => {
  const original = 'const [read, set] = createSignal(1);'; assert.equal(entry.source.split(original).length, 2);
  return { ...entry, id: entry.id + '-permitted',
    source: entry.source.replace(original, 'const [read, set] = createSignal(1, { ownedWrite: true });'),
    provenance: { ...entry.provenance, signalOwnedWrite: true } };
});
