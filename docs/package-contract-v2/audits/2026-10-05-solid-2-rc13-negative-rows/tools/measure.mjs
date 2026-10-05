// Collect the closure measurements the audit document quotes, into measurements.json.
import fs from 'node:fs';
import { load, reach, pathTo } from './closure.mjs';
const cache = {};
const L = (v, b) => (cache[v + b] ??= load(v, b));
const runs = [
  // [label, roots, cut]
  ['action', ['action'], 'run,_evaluate,B,wireExternalSource,write'],
  ['createMemo', ['createMemo'], 'flush,wireExternalSource,write'],
  ['createOptimistic', ['createOptimistic'], 'flush,wireExternalSource,write'],
  ['createOptimistic setter (setSignal)', ['setSignal'], 'flush,wireExternalSource,write'],
  ['createSignal', ['createSignal'], 'flush,wireExternalSource,write'],
  ['createSignal setter (setMemo)', ['setMemo'], 'flush,wireExternalSource,write'],
  ['createTrackedEffect', ['createTrackedEffect'], 'flush,wireExternalSource,write'],
  ['onSettled', ['onSettled'], 'flush,wireExternalSource,write'],
  ['flush', ['flush'], 'run,_evaluate,B,wireExternalSource,write'],
  ['getOwner', ['getOwner'], 'flush'],
  ['onCleanup', ['onCleanup'], 'flush'],
  ['runWithOwner', ['runWithOwner'], 'flush'],
  ['untrack', ['untrack'], 'flush'],
  ['reconcile', ['reconcile'], 'flush,wireExternalSource,write'],
  ['mapArray (For, solid-js client)', ['mapArray'], 'flush,wireExternalSource,write'],
  ['repeat (Repeat, solid-js client)', ['repeat'], 'flush,wireExternalSource,write'],
];
const out = { reads: {}, host: {} };
for (const [label, roots, cut] of runs) {
  out.reads[label] = { cut };
  for (const b of ['prod', 'observe', 'dev']) {
    const r9 = reach(L('rc9', b), roots, { cut: new Set(cut.split(',')) });
    const r13 = reach(L('rc13', b), roots, { cut: new Set(cut.split(',')) });
    out.reads[label][b] = { rc9: r9.seen.size, rc13: r13.seen.size, rc9Reads: [...r9.readCalls], rc13Reads: [...r13.readCalls], rc13Path: [...r13.readCalls].map(e => pathTo(r13.parent, e.split('->')[0])) };
  }
}
for (const r of ['action', 'createMemo', 'createOptimistic', 'createOptimisticStoreNext', 'createProjectionNext', 'createRoot', 'createSignal', 'createStore', 'createTrackedEffect', 'flush', 'getOwner', 'omit', 'onCleanup', 'onSettled', 'reconcile', 'runWithOwner', 'snapshot', 'untrack']) {
  out.host[r] = {};
  for (const b of ['prod', 'observe', 'dev']) {
    const r13 = reach(L('rc13', b), [r]);
    const r9 = reach(L('rc9', b), [r]);
    out.host[r][b] = { rc13: Object.fromEntries([...r13.host].map(([k, v]) => [k, [...v].sort()])), rc9: Object.fromEntries([...r9.host].map(([k, v]) => [k, [...v].sort()])), fns: [r9.seen.size, r13.seen.size] };
  }
}
fs.writeFileSync(process.cwd() + '/docs/package-contract-v2/audits/2026-10-05-solid-2-rc13-negative-rows/tools/measurements.json', JSON.stringify(out, null, 1));
for (const [k, v] of Object.entries(out.reads)) console.log(k.padEnd(40), ['prod', 'observe', 'dev'].map(b => `${b} ${v[b].rc9}->${v[b].rc13} reads:${v[b].rc13Reads.join(',') || 'none'}`).join(' | '));
