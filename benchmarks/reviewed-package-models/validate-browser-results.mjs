// Replay checks on saved observations. No browser or package installation is
// required; changed executable consumers and dependency bytes refuse.
import assert from "node:assert/strict";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { closurePins, hash, packageRoot, read } from "./catalog.mjs";
import holdouts from "./holdout-cases.mjs";
const [output, ...inputs] = process.argv.slice(2); assert(output && inputs.length && !existsSync(output));
const reports = inputs.map(path => ({ path: resolve(path), report: read(path) }));
const rows = new Map(); let verified = 0;
for (const { path, report } of reports) {
  assert.equal(report.authority, false); assert(report.finishedAt);
  assert.equal(report.bridgeSha256, hash(readFileSync(new URL('./runtime-feedback.mjs', import.meta.url))));
  for (const row of report.results) {
    const root = join(dirname(path), row.id), main = join(root, row.mountResultExposedForDisposal || row.id === 'helge-app' ? 'src/index.tsx' : 'src/main.tsx');
    assert.equal(hash(readFileSync(main)), row.sourceSha256); assert.equal(row.publishedTypingErrors.length, 0, row.id);
    assert.equal(row.pageErrors.length, 0, row.id); assert(!row.harnessFailure, row.harnessFailure?.message);
    for (const packageInput of row.packagePins) assert.deepEqual(closurePins(packageRoot(root, packageInput.package)), packageInput.pins);
    for (const feedback of row.feedback) {
      assert.equal(feedback.certification, false); assert.equal(feedback.basis, 'runtime-observation');
      assert(feedback.location); assert(feedback.originalLocation);
      const lines = readFileSync(feedback.originalLocation.path, 'utf8').split('\n');
      assert(feedback.originalLocation.line > 0 && feedback.originalLocation.line <= lines.length);
      assert(feedback.originalLocation.column > 0 && feedback.originalLocation.column <= lines[feedback.originalLocation.line - 1].length + 1);
    }
    rows.set(row.id, row); verified++;
  }
}
const get = id => { assert(rows.has(id), `Missing scenario: ${id}`); return rows.get(id); };
const codes = id => get(id).feedback.map(row => row.code);
assert.deepEqual(codes('helge-app'), []); assert.equal(get('helge-app').disposals, 1); assert(get('helge-app').steps.some(s => s.label === 'dispose-app'));
assert.deepEqual(codes('router-setup'), ['STRICT_READ_UNTRACKED']); assert.deepEqual(codes('router-memo'), []);
for (const id of ['router-effect', 'router-event']) {
  assert(get(id).errors.length > 0); assert(get(id).errors.every(error => error.message.includes('Context can only be accessed') && error.originalLocation));
  assert.deepEqual(codes(id), []);
}
for (const phase of ['module', 'setup', 'memo', 'effect', 'event']) {
  const id = `timer-${phase}`, row = get(id), leaks = ['module', 'effect', 'event'].includes(phase);
  assert.deepEqual(codes(id), leaks ? ['NO_OWNER_CLEANUP'] : []);
  assert.equal(row.disposals, 1);
  assert.equal(row.values.ticksAfterDispose > row.values.ticksAtDispose, leaks);
}
assert.deepEqual(codes('pagination-tracked'), ['STRICT_READ_UNTRACKED']); assert.equal(get('pagination-tracked').values.page, 2);
assert.deepEqual(codes('pagination-eager'), ['STRICT_READ_UNTRACKED', 'STRICT_READ_UNTRACKED']); assert.equal(get('pagination-eager').values.page, 1);
assert(get('pagination-tracked').feedback[0].frames.some(frame => frame.path.includes('/@solid-primitives/pagination/')));
assert(get('pagination-eager').feedback[1].attribution === 'app-frame-without-package-origin');
assert.deepEqual(codes('corvu-dialog'), []); assert.equal(get('corvu-dialog').disposals, 1);
assert.equal(get('query-local').diagnosticSubscription, false); assert.equal(get('query-local').values.fetches, 2); assert.equal(get('query-local').values.subscribersAfterDispose, 0);
const byRule = { 'missing-owner': 'NO_OWNER_EFFECT', 'strict-read-untracked': 'STRICT_READ_UNTRACKED' };
for (const entry of holdouts) for (const twin of ['misuse', 'correct']) {
  if (!entry[twin]) continue;
  assert.deepEqual(codes(`${entry.id}-${twin}`), twin === 'misuse' && entry.expectWarning !== false ? [byRule[entry.rule]] : []);
}
assert.deepEqual(codes('map-snapshot-challenge'), []); assert.deepEqual(codes('map-tracked-challenge'), []);
assert.deepEqual(get('map-snapshot-challenge').values, { renderedSize: '0', actualSize: 1 });
assert.deepEqual(get('map-tracked-challenge').values, { renderedSize: '1', actualSize: 1 });
const summary = { authority: false, reportPaths: inputs.map(path => resolve(path)), verifiedObservations: verified, uniqueScenarios: rows.size,
  originalSourceLocations: [...rows.values()].flatMap(row => row.feedback).length, frozenHoldoutObservations: 16,
  runtimeSubscriptionMissing: ['query-local'], silentStaleState: ['map-snapshot-challenge'], verifiedAt: new Date().toISOString() };
writeFileSync(output, JSON.stringify(summary, null, 2) + '\n'); console.log(JSON.stringify(summary));
