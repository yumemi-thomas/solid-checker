// Authenticate every guard premise before joining it to a consumer snapshot.
// The first executed v2 profile is preserved separately.
import { existsSync, readFileSync } from 'node:fs';
import { hash } from './catalog.mjs';
import { instrumentGuards } from './guard-trace.mjs';
import { observedGetterSnapshots, snapshotFeedbackV2 } from './snapshot-feedback-v2.mjs';
export { classSnapshotFlowsV2 } from './snapshot-feedback-v2.mjs';
const cached = new Map();
export function authenticatedGetterSnapshots(program, source, events) {
  const accepted = [], open = [];
  for (const event of events ?? []) {
    if (event.kind !== 'tracking-skipped') continue;
    if (!existsSync(event.path)) { open.push({ reason: 'guard source unavailable' }); continue; }
    const text = readFileSync(event.path, 'utf8'), digest = hash(text);
    if (digest !== event.sourceSha256) { open.push({ reason: 'guard source bytes changed' }); continue; }
    const key = event.path + ':' + digest;
    if (!cached.has(key)) cached.set(key, instrumentGuards(text, event.path)?.observations ?? []);
    const exact = cached.get(key).some(premise => premise.kind === event.kind && premise.start === event.start &&
      premise.end === event.end && premise.sourceSha256 === digest);
    if (exact) accepted.push(event); else open.push({ reason: 'guard premise does not match an exact core source guard' });
  }
  const result = observedGetterSnapshots(program, source, accepted);
  return { ...result, open: [...result.open, ...open] };
}
export const snapshotFeedbackV3 = snapshotFeedbackV2;
