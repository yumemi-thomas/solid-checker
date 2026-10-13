// Refine finite observations; a clean error channel alone does not establish
// valid callback data, successful decoding, or a package as callback caller.
import assert from 'node:assert/strict';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { hash, read } from './catalog.mjs';
const args = process.argv.slice(2).map(path => resolve(path));
assert.equal(args.length, 7);
const [surface, origins, permission, timing, payload, delivery] = args.slice(0, -1).map(read), output = args.at(-1);
assert(!existsSync(output));
const key = row => row.package + '\0' + row.export;
const baseline = new Map(surface.results.map(row => [key(row), row]));
const attributed = origins.results.filter(row => row.dependencyCallerObserved);
assert.equal(attributed.length, permission.summary.permittedWritesSucceeded);
for (const row of attributed) {
  assert(baseline.get(key(row)).confirmedWritePair);
  assert(permission.results.some(control => key(control) === key(row) && control.permittedWriteSucceeded));
}
function executions(report) {
  assert.equal(report.summary.excludedPairs, 0);
  for (const row of report.results) {
    assert(row.admitted && row.executed && row.controlClean && row.callbackObserved);
    for (const role of ['read', 'write']) {
      const observation = row.roles[role];
      assert(observation.samples.length && observation.samples.every(s => !s.owned && !s.tracked));
      assert(!observation.codes.length && !observation.errors.length && !observation.pageErrors.length &&
        !observation.consoleErrors.length && !observation.blockedRequests.length && !observation.harnessFailure && !observation.saturated);
    }
  }
  return report.studies.flatMap(study => read(join(study.path, 'browser/results.json')).results);
}
const timingExecutions = executions(timing);
for (const row of timing.results) assert(baseline.get(key(row)).confirmedWritePair);
const payloadExecutions = executions(payload);
for (const row of payloadExecutions) {
  assert(row.values.exactPayloadRestored && row.values.callbackSucceeded && row.values.callbackError === null);
  assert.deepEqual(row.values.callbackPayload, row.provenance.export === 'unzip' ? { sample: [1, 2] } : [1, 2]);
}
const deliveryExecutions = executions(delivery);
const deliveryResults = delivery.results.map(row => {
  const pair = deliveryExecutions.filter(r => r.packagePins[0].package === row.package && r.provenance.export === row.export);
  assert.equal(pair.length, 2);
  assert.equal(pair[0].values.callbackSucceeded, pair[1].values.callbackSucceeded);
  assert.equal(pair[0].values.callbackError, pair[1].values.callbackError);
  return { package: row.package, export: row.export, callbackSucceeded: pair[0].values.callbackSucceeded,
    callbackError: pair[0].values.callbackError, writeAllowedWithoutOwner: true };
});
assert.equal(timingExecutions.length, timing.results.length * 2);
assert.equal(payloadExecutions.length, payload.results.length * 2);
const summary = { additionalPackageInvokedApis: attributed.length,
  additionalPackages: new Set(attributed.map(r => r.package)).size,
  permittedWriteControls: permission.summary.permittedWritesSucceeded,
  trailingOnlySafeCallbacks: timing.results.length,
  workerCallbacksDelivered: delivery.results.length,
  initialBinaryDomainFailures: deliveryResults.filter(r => !r.callbackSucceeded).length,
  exactPayloadFlows: payload.results.length };
writeFileSync(output, JSON.stringify({ authority: false, certification: false,
  basis: 'finite typed package callback errors, successful permission controls, timing counterexamples and exact decoded payloads',
  inputs: args.slice(0, -1).map(path => ({ path, sha256: hash(readFileSync(path)) })), summary, deliveryResults }, null, 2) + '\n');
console.log(JSON.stringify(summary, null, 2));
