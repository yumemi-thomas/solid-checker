import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import cases from './cross-package-cases.mjs';
import { hash, read } from './catalog.mjs';
const selection = read(process.env.REVIEWED_MODEL_INPUT_SELECTION);
export default cases.map(entry => {
  const row = selection.rows.find(r => r.package === entry.package && r.export === entry.provenance.export);
  for (const declaration of row.protocol.declarations) {
    // The browser harness performs final published typing of the member call.
    assert.equal(hash(readFileSync(declaration.path)), declaration.sha256);
  }
  const statement = `h.attempt('protocol', () => { if (value) return value${row.protocol.members.map(m => `[${JSON.stringify(m)}]`).join('')}(${row.protocol.arguments.join(', ')}); });`;
  const marker = 'return dispose; });'; assert(entry.source.includes(marker));
  return { ...entry, id: entry.id + '-validation', source: entry.source.replace(marker, statement + marker),
    provenance: { ...entry.provenance, validationProtocol: row.protocol } };
});
