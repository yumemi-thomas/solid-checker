// Compare validated observations of identical callbacks in two caller scopes.
// A successful imperative write says nothing about cleanup or unexecuted paths.
import assert from 'node:assert/strict';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { hash, read } from './catalog.mjs';

const [selectionArg, imperativeArg, outputArg] = process.argv.slice(2);
const selectionPath = resolve(selectionArg), imperativePath = resolve(imperativeArg), output = resolve(outputArg);
assert(!existsSync(output));
const selection = read(selectionPath), imperative = read(imperativePath), owned = read(selection.historyPath);
assert.equal(hash(readFileSync(selection.historyPath)), selection.historySha256);
assert.equal(hash(readFileSync(selection.parentSelectionPath)), selection.parentSelectionSha256);
assert.equal(imperative.summary.excludedPairs, 0);
assert.equal(imperative.results.length, selection.rows.length);
const results = selection.rows.map(row => {
  const matches = report => report.results.filter(r => r.package === row.package && r.export === row.export);
  const baseline = matches(owned), comparison = matches(imperative);
  assert.equal(baseline.length, 1); assert.equal(comparison.length, 1);
  const before = baseline[0], after = comparison[0];
  assert(before.confirmedWritePair && before.observedOnlyWithoutObserver);
  assert(after.admitted && after.executed && after.callbackObserved);
  const ids = new Set(before.mappedCallbackWrites.map(w => w.id));
  // Every callback previously implicated in the owned error must actually run
  // in the imperative observation, with neither public owner nor observer.
  for (const id of ids) assert(after.roles.write.samples.some(s => s.id === id && !s.owned && !s.tracked));
  for (const role of ['read', 'write']) {
    const observation = after.roles[role];
    assert(!observation.saturated && !observation.errors.length && !observation.pageErrors.length &&
      !observation.consoleErrors.length && !observation.blockedRequests.length && !observation.harnessFailure);
    assert(!observation.codes.includes('REACTIVE_WRITE_IN_OWNED_SCOPE'));
  }
  return { package: row.package, export: row.export, ownedWriteDiagnosed: true,
    imperativeWriteSucceeded: true, imperativeReadClean: after.controlClean,
    imperativeCodes: after.roles.write.codes, previouslyDiagnosedCallbackIds: [...ids] };
});
const summary = { exports: results.length, packages: new Set(results.map(r => r.package)).size,
  writesAllowedImperatively: results.filter(r => r.imperativeWriteSucceeded).length,
  fullyCleanImperativeControls: results.filter(r => r.imperativeReadClean).length,
  imperativeControlsWithSeparateFeedback: results.filter(r => r.imperativeCodes.length).length };
writeFileSync(output, JSON.stringify({ authority: false, certification: false,
  basis: 'finite paired callback executions in owned and imperative caller scopes',
  inputs: [selectionPath, imperativePath, selection.historyPath].map(path => ({ path, sha256: hash(readFileSync(path)) })),
  summary, results }, null, 2) + '\n');
console.log(JSON.stringify(summary, null, 2));
