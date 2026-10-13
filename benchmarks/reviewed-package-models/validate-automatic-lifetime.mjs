import assert from 'node:assert/strict';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { hash, read } from './catalog.mjs';
import cases from './automatic-lifetime-cases.mjs';
const [studyArg, outputArg] = process.argv.slice(2), study = resolve(studyArg), output = resolve(outputArg);
assert(!existsSync(output));
const before = read(join(study, 'inputs-before.json')), after = read(join(study, 'inputs-after.json'));
assert.deepEqual(after.files, before.files);
for (const input of before.files) {
  assert.equal(hash(readFileSync(input.path)), input.sha256, input.path);
  assert.equal(hash(readFileSync(join(study, 'source-inputs', input.sha256.slice(7) + '.mjs'))), input.sha256);
}
const names = existsSync(join(study, 'events/results.json')) ? ['original', 'events', 'lowered', 'native'] : ['original', 'native'];
const comparisons = names.filter(name => name !== 'original');
const profiles = Object.fromEntries(names.map(profile => [profile, read(join(study, profile, 'results.json'))]));
const lifetime = row => row.feedback.filter(item => item.code === 'RESOURCE_OUTLIVES_DECLARED_SCOPE');
const core = row => row.feedback.filter(item => item.code !== 'RESOURCE_OUTLIVES_DECLARED_SCOPE').map(item => ({ code: item.code, severity: item.severity }));
const behaviors = row => { const { resourceAudit, automaticState, ...values } = row.values ?? {}; return values; };
const results = [], totals = {};
for (const [profile, report] of Object.entries(profiles)) {
  assert.equal(report.authority, false); assert(report.finishedAt);
  assert.equal(report.results.length, cases.length + (profile === 'original' || profile === 'native' ? 1 : 0));
  for (const row of report.results) {
    assert.equal(row.excludedBeforeExecution, undefined); assert.equal(row.publishedTypingErrors.length, 0, row.id);
    assert.equal(row.attributionEnabled, true, row.id); assert.equal(row.attributionState.installed, true);
    if (profile !== 'lowered') { assert.equal(row.harnessFailure, undefined, row.id); assert.equal(row.pageErrors.length, 0, row.id); }
    if (profile !== 'original') {
      assert.equal(row.sourceInstrumentation.refused.length, 0, row.id);
      for (const transform of row.sourceInstrumentation.transformed) for (const event of transform.events) {
        assert.equal(hash(readFileSync(event.path)), event.sourceSha256);
        if (row.id !== 'helge-app') assert.equal(event.property, 'onClick');
        for (const declaration of event.declarations) assert.equal(hash(readFileSync(declaration.path)), declaration.sha256);
      }
    }
  }
  const targets = report.results.filter(row => row.provenance?.role === 'target'), controls = report.results.filter(row => row.provenance?.role === 'control');
  totals[profile] = { targets: targets.length, detected: targets.filter(row => lifetime(row).length).length,
    controls: controls.length, controlsWithLifetimeWarning: controls.filter(row => lifetime(row).length).length,
    failures: report.results.filter(row => row.harnessFailure || row.pageErrors.length).map(row => row.id) };
  assert.equal(totals[profile].controlsWithLifetimeWarning, 0);
}
for (const entry of cases) {
  const rows = Object.fromEntries(Object.entries(profiles).map(([profile, report]) => [profile, report.results.find(row => row.id === entry.id)]));
  const original = rows.original, native = rows.native;
  for (const row of Object.values(rows)) { assert.equal(row.sourceSha256, original.sourceSha256); assert.deepEqual(row.packagePins, original.packagePins); }
  assert.deepEqual(core(native), core(original), entry.id);
  const expected = entry.provenance.expectedIssue && !entry.provenance.nativeMiss;
  assert.equal(lifetime(native).length > 0, expected, entry.id);
  for (const finding of lifetime(native)) {
    assert.equal(finding.certification, false); assert.equal(finding.severity, 'warning');
    assert.equal(finding.resourceKind, entry.provenance.expectedKind);
    assert(finding.originalLocation?.path.startsWith(join(study, 'native', entry.id, 'src/')), entry.id);
  }
  // Preserve raw values and their comparison. Native timer counts depend on
  // elapsed browser time; they are observations, not a deterministic oracle.
  const values = Object.fromEntries(Object.entries(rows).map(([profile, row]) => [profile, behaviors(row)]));
  const parity = Object.fromEntries(comparisons.map(profile => [profile, {
    valuesEqual: JSON.stringify(values[profile]) === JSON.stringify(values.original),
    coreEqual: JSON.stringify(core(rows[profile])) === JSON.stringify(core(original)),
    executionCompleted: !rows[profile].harnessFailure && !rows[profile].pageErrors.length,
  }]));
  if (entry.provenance.schedulingControl) assert.deepEqual(values.native.order, values.original.order, entry.id);
  if (!entry.provenance.sharedResource || entry.provenance.expectedKind !== 'interval') assert.deepEqual(values.native, values.original, entry.id);
  results.push({ id: entry.id, package: entry.package, provenance: entry.provenance, values, parity,
    observed: Object.fromEntries(Object.entries(rows).map(([profile, row]) => [profile, { lifetimeWarnings: lifetime(row).length,
      coreFeedback: core(row), pageErrors: row.pageErrors, harnessFailure: row.harnessFailure ?? null,
      locations: lifetime(row).map(item => item.originalLocation), gaps: row.sourceInstrumentation?.transformed.flatMap(item => item.gaps) ?? [] }])) });
}
const originalApp = profiles.original.results.find(row => row.id === 'helge-app'), nativeApp = profiles.native.results.find(row => row.id === 'helge-app');
assert.equal(nativeApp.sourceSha256, originalApp.sourceSha256); assert.deepEqual(nativeApp.packagePins, originalApp.packagePins);
assert.deepEqual(core(nativeApp), core(originalApp));
const report = { authority: false, certification: false, study, frozenFiles: before.files.length, inputsUnchanged: true,
  consumerCases: cases.length, packages: [...new Set(cases.map(entry => entry.package))], totals, results,
  app: { id: 'helge-app', coreEqual: true, steps: nativeApp.steps.map(step => step.label),
    lifetimeWarnings: lifetime(nativeApp), admittedEvents: nativeApp.sourceInstrumentation.transformed.reduce((sum, item) => sum + item.events.length, 0),
    runtimeGaps: nativeApp.values?.automaticState?.gaps ?? null,
    sourceGaps: nativeApp.sourceInstrumentation.transformed.flatMap(item => item.gaps) },
  nativeSchedulingParity: true, loweredProfileSuitable: profiles.lowered ? false : null };
if (profiles.lowered) assert(totals.lowered.failures.length > 0, 'Keep the rejected transformation failure visible');
writeFileSync(output, JSON.stringify(report, null, 2) + '\n');
console.log(JSON.stringify({ consumerCases: report.consumerCases, packages: report.packages.length, totals, nativeSchedulingParity: true, appEvents: report.app.admittedEvents, frozenFiles: before.files.length }, null, 2));
