// Capture actual decoded bytes without replacing any package code or type.
import assert from 'node:assert/strict';
import cases from './binary-delivery-cases.mjs';
export default cases.map(entry => {
  const marker = 'h.values.callbackDataPresent = data !== undefined, undefined'; assert(entry.source.includes(marker));
  const helper = `function recordPayload(data: unknown): unknown {
    if (data instanceof Uint8Array) return Array.from(data);
    if (data && typeof data === 'object') return Object.fromEntries(Object.entries(data).map(([key, value]) =>
      [key, value instanceof Uint8Array ? Array.from(value) : value]));
    return data;
  }\n`;
  const source = helper + entry.source.replace(marker, 'h.values.callbackDataPresent = data != null, h.values.callbackPayload = recordPayload(data), undefined');
  return { ...entry, id: entry.id + '-payload', source,
    flow: async page => { await entry.flow(page); await page.evaluate(() => { const h = globalThis.__experiment;
      const expected = Array.isArray(h.values.callbackPayload) ? [1, 2] : { sample: [1, 2] };
      if (!h.values.callbackSucceeded || h.values.callbackError !== null || JSON.stringify(h.values.callbackPayload) !== JSON.stringify(expected))
        throw new Error('Decoder did not restore the exact supplied payload');
      h.values.exactPayloadRestored = true;
    }); }, provenance: { ...entry.provenance, expectedPayloadBytes: [1, 2] } };
});
