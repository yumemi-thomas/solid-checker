// Keep construction observations, source assumptions and runtime failures
// distinct. Adaptive inputs are discovery evidence, not certified contracts.
import assert from 'node:assert/strict';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { authenticateModel, hash, read } from './catalog.mjs';
const [selectionArg, outputArg, ...studyArgs] = process.argv.slice(2), selection = read(resolve(selectionArg)), output = resolve(outputArg);
assert(studyArgs.length && !existsSync(output)); const catalog = read(selection.catalogPath); assert.equal(hash(readFileSync(selection.catalogPath)), selection.catalogSha256);
const observations = [], studies = [];
for (const argument of studyArgs) {
  const path = resolve(argument), before = read(join(path, 'inputs-before.json')), after = read(join(path, 'inputs-after.json'));
  assert.deepEqual(before.files, after.files); const currentInputChanges = [];
  for (const input of before.files) {
    assert.equal(hash(readFileSync(join(path, 'source-inputs', input.sha256.slice(7)))), input.sha256);
    if (!existsSync(input.path) || hash(readFileSync(input.path)) !== input.sha256) currentInputChanges.push(input.path);
  }
  const local = read(before.selectionPath), report = read(join(path, 'browser/results.json'));
  assert(report.finishedAt && report.authority === false); assert.equal(report.results.length, local.rows.length * 2);
  if (local.parentSelectionPath) assert.equal(hash(readFileSync(local.parentSelectionPath)), local.parentSelectionSha256);
  if (local.parentBrowserPath) assert.equal(hash(readFileSync(local.parentBrowserPath)), local.parentBrowserSha256);
  if (local.historyPath) assert.equal(hash(readFileSync(local.historyPath)), local.historySha256);
  for (const row of local.rows) {
    authenticateModel(catalog.models.find(m => m.package === row.package), row.project);
    for (const declaration of row.declaration) assert.equal(hash(readFileSync(declaration.path)), declaration.sha256);
    const pair = report.results.filter(r => r.provenance.export === row.export && r.packagePins[0].package === row.package); assert.equal(pair.length, 2);
    for (const result of pair) {
      assert.deepEqual(result.provenance.arguments, row.arguments); assert.deepEqual(result.packagePins[0].pins, row.pins);
      assert.equal(result.publishedTypingErrors.length, 0); assert.equal(result.excludedBeforeExecution, undefined);
      assert(result.attributionEnabled && result.runtime.every(r => r.version === '2.0.0-rc.9'));
    }
    const unowned = pair.find(r => r.provenance.phase === 'unowned'), owned = pair.find(r => r.provenance.phase === 'owned');
    const failure = r => r.errors.length || r.pageErrors.length || r.consoleErrors.length || r.harnessFailure || r.blockedRequests.length;
    const moduleExecuted = pair.every(r => [...r.observations, ...r.errors].some(o => o.label === 'invoke'));
    const ownerDetected = unowned.feedback.some(f => ['NO_OWNER_CLEANUP', 'NO_OWNER_EFFECT'].includes(f.code));
    const ownedExecutionWarnings = owned.feedback.filter(f => f.category === 'execution');
    observations.push({ package: row.package, version: row.version, export: row.export, study: path, arguments: row.arguments,
      sourceOwnerAssumption: row.sourcePremise.owner, moduleExecuted, ownerDetected,
      confirmedPair: moduleExecuted && ownerDetected && !pair.some(failure) && !ownedExecutionWarnings.length,
      ownedAdvisories: owned.feedback.filter(f => f.category === 'advisory').map(f => f.code),
      failures: pair.map(r => ({ phase: r.provenance.phase, errors: r.errors, pageErrors: r.pageErrors, consoleErrors: r.consoleErrors,
        harnessFailure: r.harnessFailure ?? null, blockedRequests: r.blockedRequests, codes: r.feedback.map(f => f.code) })) });
  }
  studies.push({ path, frozenFiles: before.files.length, sourceEvidence: 'execution of the preserved source bytes', currentInputChanges, executions: report.results.length });
}
const key = row => `${row.package}\0${row.export}`, confirmed = new Map();
for (const row of observations) if (row.confirmedPair && !confirmed.has(key(row))) confirmed.set(key(row), row);
const rows = selection.rows.map(row => ({ package: row.package, export: row.export, confirmed: confirmed.has(key(row)),
  attempts: observations.filter(o => key(o) === key(row)) }));
const selected = selection.rows.filter(row => confirmed.has(key(row))).map(row => ({ ...row, arguments: confirmed.get(key(row)).arguments,
  behavioralWitness: { study: confirmed.get(key(row)).study, authority: false, constructorOnly: true } }));
const successfulPath = output.replace(/\.json$/, '-selection.json'); assert(!existsSync(successfulPath));
writeFileSync(successfulPath, JSON.stringify({ ...selection, rows: selected, parentSelectionPath: resolve(selectionArg), parentSelectionSha256: hash(readFileSync(resolve(selectionArg))),
  summary: { selectedExports: selected.length, selectedPackages: new Set(selected.map(r => r.package)).size }, basis: 'type-generated inputs with observed unowned diagnostic and a valid owned control' }, null, 2) + '\n');
const report = { authority: false, certification: false, selection: selection.summary, studies,
  summary: { admittedExports: selection.rows.length, attemptedInputs: observations.length, browserExecutions: observations.length * 2,
    ownershipDiagnosticObserved: rows.filter(r => r.attempts.some(a => a.ownerDetected)).length,
    confirmedOwnershipPairs: confirmed.size, confirmedPackages: new Set([...confirmed.values()].map(r => r.package)).size,
    ownedControlsWithAdvisories: [...confirmed.values()].filter(r => r.ownedAdvisories.length).length,
    unverifiedExports: rows.filter(r => !r.confirmed).length, publishedTypingErrors: 0 }, successfulSelectionPath: successfulPath,
  rows, synthesisRefusals: selection.refusals };
writeFileSync(output, JSON.stringify(report, null, 2) + '\n'); console.log(JSON.stringify(report.summary, null, 2));
