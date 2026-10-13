import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { authenticateModel, hash, read } from './catalog.mjs';
import { instrumentCallbackArguments } from './callback-instrument.mjs';
const path = process.env.REVIEWED_MODEL_INPUT_SELECTION; assert(path);
const selection = read(path), catalog = read(selection.catalogPath); assert.equal(hash(readFileSync(selection.catalogPath)), selection.catalogSha256);
const authenticated = new Set(), cases = [];
for (const row of selection.rows) {
  if (!authenticated.has(row.package)) { authenticateModel(catalog.models.find(m => m.package === row.package), row.project); authenticated.add(row.package); }
  for (const mode of ['read', 'write']) {
    const instrumented = instrumentCallbackArguments(row.arguments, mode); assert.equal(instrumented.count, row.callbacks.length);
    const args = instrumented.arguments;
    const consume = row.consume === 'self' ? `if (typeof value === 'function') h.attempt('consume', () => untrack(() => value()));` :
      row.consume === 'tuple0' ? `if (value) h.attempt('consume', () => untrack(() => value[0]()));` : '';
    const id = `callback-${row.package.slice('@solid-primitives/'.length)}-${row.export}-${hash(row.export).slice(7,15)}-${mode}`;
    const source = `import { createRoot, createSignal, flush, getOwner, getObserver, untrack } from 'solid-js';
import { ${JSON.stringify(row.export)} as candidate } from ${JSON.stringify(row.package)};
const h = (globalThis as any).__experiment; h.samples = []; h.stage = 'construct';
h.dispose = createRoot(dispose => { const [read, set] = createSignal(1); h.change = () => set(2);
const invoke = () => candidate(${args.join(', ')}); let result: ReturnType<typeof invoke> | undefined;
h.attempt('invoke', () => { result = invoke(); }); const value = result; h.stage = 'consume'; ${consume} return dispose; });
h.attempt('flush', () => flush()); h.ready = true;`;
    cases.push({ id, package: row.package, source, patchCopy() { return { selection: row }; },
      flow: async page => { await page.waitForTimeout(30); await page.evaluate(() => { const h = globalThis.__experiment;
        if (!h.ready) throw new Error('Consumer module did not execute'); h.stage = 'change'; h.attempt('change', h.change); });
        await page.waitForTimeout(30); await page.evaluate(() => { const h = globalThis.__experiment; h.values.callbackSamples = h.samples; h.values.saturated = !!h.saturated;
          h.values.lastRead = h.lastRead; h.attempt('dispose', h.dispose); h.disposals++; }); },
      provenance: { role: mode, export: row.export, arguments: row.arguments, callbacks: row.callbacks, consume: row.consume, sourceContextAssumptions: row.callbackSource.assumptions } });
  }
}
export default cases;
