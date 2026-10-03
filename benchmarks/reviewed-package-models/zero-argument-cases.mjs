import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { authenticateModel, hash, read } from './catalog.mjs';
const path = process.env.REVIEWED_MODEL_ZERO_ARGUMENT_SELECTION; assert(path, 'Provide the frozen zero-argument selection');
const selection = read(path); assert.equal(hash(readFileSync(selection.catalogPath)), selection.catalogSha256);
const catalog = read(selection.catalogPath), cases = [], authenticated = new Set();
for (const row of selection.rows) if (!authenticated.has(row.package)) {
  authenticateModel(catalog.models.find(model => model.package === row.package), row.project); authenticated.add(row.package);
}
for (const row of selection.rows) for (const phase of ['unowned', 'owned']) {
  const id = `zero-${row.package.slice('@solid-primitives/'.length)}-${row.export}-${phase}`;
  const prelude = `import { createRoot, flush } from 'solid-js'; import { ${JSON.stringify(row.export)} as candidate } from ${JSON.stringify(row.package)};
const h = (globalThis as any).__experiment; const invoke = () => h.attempt('invoke', () => { const value = candidate(); h.values.returnType = typeof value; });`;
  const body = phase === 'owned' ? `h.dispose = createRoot(dispose => { invoke(); return dispose; });` : `invoke(); h.dispose = () => {};`;
  cases.push({ id, package: row.package, source: prelude + '\n' + body + ` h.attempt('flush', () => flush());`,
    patchCopy() { return { selection: row }; },
    flow: async (page, step) => { await page.waitForTimeout(30); await step('dispose', () => page.evaluate(() => { const h = globalThis.__experiment; h.attempt('dispose', h.dispose); h.disposals++; })); await page.waitForTimeout(30); },
    provenance: { automaticSelection: true, phase, export: row.export, sourceOwnerAssumption: row.sourcePremise.owner,
      declarations: row.declaration, zeroArgumentSignatures: row.zeroArgumentSignatures, role: 'observation' } });
}
export default cases;
