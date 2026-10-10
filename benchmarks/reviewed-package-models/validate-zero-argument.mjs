import assert from 'node:assert/strict';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { authenticateModel, hash, read } from './catalog.mjs';
const [studyArg, staticArg, outputArg] = process.argv.slice(2), study = resolve(studyArg), output = resolve(outputArg); assert(!existsSync(output));
const before = read(join(study, 'inputs-before.json')), after = read(join(study, 'inputs-after.json'));
assert.deepEqual(before.files, after.files);
const currentInputChanges = [];
for (const file of before.files) {
  assert.equal(hash(readFileSync(join(study, 'source-inputs', file.sha256.slice(7)))), file.sha256);
  const currentSha256 = hash(readFileSync(file.path));
  if (currentSha256 !== file.sha256) currentInputChanges.push({ path: file.path, recordedSha256: file.sha256, currentSha256 });
}
const selection = read(before.selectionPath), catalog = read(selection.catalogPath), browser = read(join(study, 'browser/results.json')), statics = read(resolve(staticArg));
assert.equal(hash(readFileSync(selection.catalogPath)), selection.catalogSha256); assert.equal(statics.modelSha256, selection.catalogSha256);
assert.equal(browser.authority, false); assert(browser.finishedAt); assert.equal(statics.authority, false); assert(statics.summary);
assert.equal(browser.results.length, selection.rows.length * 2); assert.equal(statics.results.length, selection.rows.length);
const rows = [];
for (const selected of selection.rows) {
  const model = catalog.models.find(model => model.package === selected.package); authenticateModel(model, selected.project);
  for (const declaration of selected.declaration) assert.equal(hash(readFileSync(declaration.path)), declaration.sha256);
  const phases = Object.fromEntries(['unowned', 'owned'].map(phase => [phase, browser.results.find(row => row.packagePins[0].package === selected.package && row.provenance.export === selected.export && row.provenance.phase === phase)]));
  for (const [phase, row] of Object.entries(phases)) {
    assert(row); assert.equal(row.publishedTypingErrors.length, 0); assert.equal(row.excludedBeforeExecution, undefined);
    assert.equal(row.harnessFailure, undefined); assert.equal(row.pageErrors.length, 0); assert.equal(row.errors.length, 0);
    assert.equal(row.attributionEnabled, true); assert.equal(row.attributionState.installed, true);
    assert.deepEqual(row.packagePins[0].pins, selected.pins);
    if (phase === 'owned') assert.equal(row.feedback.length, 0);
    else assert(row.feedback.some(item => ['NO_OWNER_CLEANUP', 'NO_OWNER_EFFECT'].includes(item.code)));
  }
  const result = statics.results.find(row => row.package === selected.package && row.export === selected.export); assert(result);
  const observations = result.observations.filter(row => row.host === 'browser'); assert.equal(observations.length, 2);
  for (const row of observations) {
    assert.equal(row.publishedTypingErrors, 0); assert.equal(row.analysisTwinTypingErrors, 0);
    assert(row.sites.some(site => site.applied && site.behavior?.owner), `${selected.package}.${selected.export}/${row.twin}: owner control must exercise its premise`);
    for (const warning of row.warnings) { assert.equal(warning.certification, false); assert.equal(warning.severity, 'warning'); assert.equal(warning.basis, 'source-extracted-assumption'); }
    if (row.twin === 'correct') assert.equal(row.warnings.length, 0);
  }
  rows.push({ package: selected.package, export: selected.export, sourcePremise: selected.sourcePremise,
    publishedSignatures: selected.zeroArgumentSignatures,
    runtime: Object.fromEntries(Object.entries(phases).map(([phase, row]) => [phase, { codes: row.feedback.map(f => f.code), locations: row.feedback.map(f => f.originalLocation), returnType: row.values.returnType }])),
    staticDetected: observations.find(row => row.twin === 'misuse').warnings.some(w => w.rule === 'missing-owner'),
    staticWarnings: observations.map(row => ({ twin: row.twin, warnings: row.warnings, unsupported: row.unsupported, baseline: row.baseline })) });
}
const report = { authority: false, certification: false, sourceInventory: catalog.summary, selection: selection.summary, frozenBrowserFiles: before.files.length,
  browserEvidenceBasis: 'historical execution of the preserved frozen source bytes', currentInputChanges,
  summary: { exports: rows.length, packages: new Set(rows.map(row => row.package)).size, runtimeDetected: rows.length, ownedRuntimeClean: rows.length,
    staticDetected: rows.filter(row => row.staticDetected).length, ownedStaticClean: rows.length, typingErrors: 0, browserEnvironmentFailures: 0 },
  rows, unexercised: selection.refusals, binaryInputs: { checkerSha256: statics.checkerSha256, typefactsSha256: statics.typefactsSha256, typescript: statics.typescript } };
writeFileSync(output, JSON.stringify(report, null, 2) + '\n'); console.log(JSON.stringify(report.summary, null, 2));
