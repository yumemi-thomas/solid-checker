// Keep member receivers and import interop explicit in the generated consumer.
import assert from 'node:assert/strict';
import { packageDigest, read } from './catalog.mjs';
import cases from './cross-package-cases.mjs';
const selection = read(process.env.REVIEWED_MODEL_INPUT_SELECTION), authenticated = new Set();
export default cases.map(entry => {
  const row = selection.rows.find(r => r.package === entry.package && r.export === entry.provenance.export);
  for (const pin of row.typingPins) if (!authenticated.has(pin.root)) {
    assert.equal(packageDigest(pin.root), pin.digest); authenticated.add(pin.root);
  }
  const original = `import { ${JSON.stringify(row.export)} as candidate } from ${JSON.stringify(row.package)};`;
  const replacement = row.importStyle === 'default' ? `import Package from ${JSON.stringify(row.package)};` : `import * as Package from ${JSON.stringify(row.package)};`;
  assert(entry.source.includes(original));
  let source = entry.source.replace(original, replacement);
  const invoke = 'const invoke = () => candidate('; assert(source.includes(invoke));
  source = source.replace(invoke, `const invoke = () => Package[${JSON.stringify(row.export)}](`);
  return { ...entry, id: entry.id + '-surface', source, provenance: { ...entry.provenance, importStyle: row.importStyle, typingPins: row.typingPins } };
});
