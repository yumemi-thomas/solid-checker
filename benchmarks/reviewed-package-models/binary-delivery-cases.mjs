import assert from 'node:assert/strict';
import cases from './surface-cases.mjs';
export default cases.map(entry => {
  const marker = "h.stage = 'consume';"; assert(entry.source.includes(marker));
  const source = entry.source.replace(marker, `${marker} h.returned = value;`);
  return { ...entry, id: entry.id + '-delivery', source,
    flow: async page => {
      await page.waitForFunction(() => globalThis.__experiment.samples.length > 0, null, { timeout: 5000 });
      await entry.flow(page);
      await page.evaluate(() => { const h = globalThis.__experiment;
        if (typeof h.returned === 'function') h.attempt('terminate-after-delivery', h.returned); });
    }, provenance: { ...entry.provenance, delayedReturnConsumption: true } };
});
