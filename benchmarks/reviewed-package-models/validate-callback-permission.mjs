// Require the previously diagnosed callbacks to execute successfully with the
// same owned caller and an explicitly permitted native signal write.
import assert from 'node:assert/strict';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { hash, read } from './catalog.mjs';
const [selectionArg, validationArg, outputArg] = process.argv.slice(2);
const selectionPath = resolve(selectionArg), validationPath = resolve(validationArg), output = resolve(outputArg);
assert(!existsSync(output));
const selection = read(selectionPath), validation = read(validationPath), baseline = read(selection.historyPath);
assert.equal(hash(readFileSync(selection.historyPath)), selection.historySha256);
assert.equal(hash(readFileSync(selection.parentSelectionPath)), selection.parentSelectionSha256);
assert.equal(validation.summary.excludedPairs, 0);
assert.equal(validation.results.length, selection.rows.length);
const results = selection.rows.map(row => {
  const find = report => report.results.filter(r => r.package === row.package && r.export === row.export);
  const before = find(baseline), after = find(validation);
  assert.equal(before.length, 1); assert.equal(after.length, 1);
  assert(before[0].confirmedWritePair);
  const result = after[0]; assert(result.admitted && result.executed && result.controlClean);
  for (const id of new Set(before[0].mappedCallbackWrites.map(w => w.id)))
    assert(result.roles.write.samples.some(s => s.id === id && s.owned));
  const written = result.roles.write;
  assert(!written.saturated && !written.codes.length && !written.errors.length && !written.pageErrors.length &&
    !written.consoleErrors.length && !written.blockedRequests.length && !written.harnessFailure);
  return { package: row.package, export: row.export, defaultWriteDiagnosed: true, permittedWriteSucceeded: true };
});
const summary = { exports: results.length, packages: new Set(results.map(r => r.package)).size,
  permittedWritesSucceeded: results.filter(r => r.permittedWriteSucceeded).length };
writeFileSync(output, JSON.stringify({ authority: false, certification: false,
  basis: 'finite callback executions with an explicit ownedWrite option on the native signal',
  inputs: [selectionPath, validationPath, selection.historyPath].map(path => ({ path, sha256: hash(readFileSync(path)) })),
  summary, results }, null, 2) + '\n');
console.log(JSON.stringify(summary, null, 2));
