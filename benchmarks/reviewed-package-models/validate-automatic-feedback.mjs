// Replay the automatic-channel and intent evidence against actual source bytes.
import assert from "node:assert/strict";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import cases from "./automatic-feedback-cases.mjs";
import { closurePins, hash, packageRoot, read } from "./catalog.mjs";
import { instrumentOrigins, instrumentSettledFire } from "./origin-trace.mjs";
const [studyArgument, inventoryArgument, outputArgument, originalArgument, tracedArgument, intentArgument, breadthArgument, priorBreadthArgument, ...deliveryArguments] = process.argv.slice(2);
assert(!process.env.REVIEWED_MODEL_AUTOMATIC_PROFILE);
const output = resolve(outputArgument); assert(!existsSync(output));
const study = read(resolve(studyArgument)), inventory = read(resolve(inventoryArgument));
for (const input of study.inputs) assert.equal(input.sha256, hash(readFileSync(new URL(input.name, import.meta.url))));
for (const entry of cases) assert.equal(study.cases.find(row => row.id === entry.id).sourceSha256, hash(entry.source));
assert.equal(inventory.inventorySha256, hash(readFileSync(new URL('./mechanism-inventory.mjs', import.meta.url))));
for (const row of inventory.results) for (const module of row.modules) assert.equal(module.sha256, hash(readFileSync(module.path)));
const originalPath = resolve(originalArgument), tracedPath = resolve(tracedArgument), intentPath = resolve(intentArgument);
const original = read(originalPath), traced = read(tracedPath), intent = read(intentPath);
const byId = report => new Map(report.results.map(row => [row.id, row]));
const originalRows = byId(original), tracedRows = byId(traced), intentRows = byId(intent);
function authenticate(path, report, expectedBridge = 'extended-runtime-feedback.mjs') {
  assert(report.finishedAt); assert.equal(report.authority, false);
  assert.equal(report.bridgeSha256, hash(readFileSync(new URL(expectedBridge, import.meta.url))));
  for (const row of report.results) {
    const sourcePath = join(dirname(path), row.id, row.mountResultExposedForDisposal ? 'src/index.tsx' : 'src/main.tsx');
    assert.equal(row.sourceSha256, hash(readFileSync(sourcePath))); assert(!row.harnessFailure);
    for (const pkg of row.packagePins) assert.deepEqual(pkg.pins, closurePins(packageRoot(dirname(sourcePath), pkg.package)));
    for (const event of row.feedback) assert.equal(event.certification, false);
    if (row.originInstrumentation) {
      assert.equal(row.originInstrumentation.instrumenterSha256, hash(readFileSync(new URL('./origin-trace.mjs', import.meta.url))));
      assert.equal(row.originInstrumentation.collectorSha256, hash(readFileSync(new URL('./origin-trace-runtime.mjs', import.meta.url))));
      for (const site of row.originInstrumentation.transformed) {
        assert.equal(site.sourceSha256, hash(readFileSync(site.path)));
        const transform = site.operation === 'onSettled-runtime-fire' ? instrumentSettledFire : instrumentOrigins;
        assert(transform(readFileSync(site.path, 'utf8'), site.path).registrations.some(candidate => candidate.start === site.start && candidate.end === site.end));
      }
    }
  }
}
authenticate(originalPath, original); authenticate(tracedPath, traced); authenticate(intentPath, intent, 'runtime-feedback.mjs');
const channelOutcomes = [];
for (const entry of cases.filter(entry => entry.provenance.profile === 'channels')) {
  const row = originalRows.get(entry.id), tracedRow = tracedRows.get(entry.id); assert(row && tracedRow);
  assert.equal(row.sourceSha256, hash(entry.source)); assert.equal(tracedRow.sourceSha256, row.sourceSha256);
  if (entry.provenance.category === 'typing-candidate' && entry.provenance.role === 'target') {
    assert(row.excludedBeforeExecution && tracedRow.excludedBeforeExecution);
    assert(row.publishedTypingErrors.some(error => error.code === 2345));
    channelOutcomes.push({ id: entry.id, excludedTypeScriptCode: 2345 }); continue;
  }
  assert.equal(row.publishedTypingErrors.length, 0); assert.equal(tracedRow.publishedTypingErrors.length, 0);
  assert(row.attributionState.installed && tracedRow.attributionState.installed, 'Engine must be live, not merely requested');
  assert.deepEqual(tracedRow.values, row.values);
  assert.deepEqual(tracedRow.feedback.map(event => [event.code, event.category, event.severity]), row.feedback.map(event => [event.code, event.category, event.severity]));
  assert.deepEqual(tracedRow.errors.map(error => error.message), row.errors.map(error => error.message));
  assert.deepEqual(tracedRow.pageErrors.map(error => error.message), row.pageErrors.map(error => error.message));
  if (entry.provenance.role === 'target') assert(row.feedback.some(event => event.code === entry.provenance.expectedCode));
  else assert.equal(row.feedback.length + row.errors.length + row.pageErrors.length, 0);
  channelOutcomes.push({ id: entry.id, category: entry.provenance.category, codes: row.feedback.map(event => event.code),
    severities: row.feedback.map(event => event.severity), attributionRuns: row.attributionState.runs.length });
}
const focusBefore = originalRows.get('automatic-autofocus-cleanup-target').feedback[0];
const focusAfter = tracedRows.get('automatic-autofocus-cleanup-target').feedback[0];
assert.equal(focusBefore.originalLocation, null); assert(!focusBefore.originalRegistration);
assert(focusAfter.originalRegistration && existsSync(focusAfter.originalRegistration.path));
const registeredSource = readFileSync(focusAfter.originalRegistration.path, 'utf8').split('\n')[focusAfter.originalRegistration.line - 1];
assert(registeredSource.slice(focusAfter.originalRegistration.column - 1).startsWith('autofocus'));
assert.equal(tracedRows.get('automatic-autofocus-cleanup-target').originFailures.length, 1);
const intentWitnesses = [];
for (const name of ['snapshot-intent', 'background-intent']) {
  const target = intentRows.get(`automatic-${name}-target`), control = intentRows.get(`automatic-${name}-control`);
  assert.equal(target.sourceSha256, control.sourceSha256); assert.equal(target.publishedTypingErrors.length + control.publishedTypingErrors.length, 0);
  assert.equal(target.feedback.length + control.feedback.length + target.errors.length + control.errors.length + target.pageErrors.length + control.pageErrors.length, 0);
  assert.deepEqual(target.guardTrace.map(note => [note.kind, note.sourceSha256, note.start]), control.guardTrace.map(note => [note.kind, note.sourceSha256, note.start]));
  assert(target.guardTrace.length); assert.equal(target.values.behavior.actual, control.values.behavior.actual);
  assert.notEqual(target.values.behavior.desired, control.values.behavior.desired);
  assert.notEqual(target.values.behavior.actual, target.values.behavior.desired); assert.equal(control.values.behavior.actual, control.values.behavior.desired);
  intentWitnesses.push({ pair: name, sourceSha256: target.sourceSha256, actual: target.values.behavior.actual,
    desiredTarget: target.values.behavior.desired, desiredControl: control.values.behavior.desired, identicalGuardKinds: target.guardTrace.map(note => note.kind) });
}
let breadthSummary = null;
if (breadthArgument) {
  const path = resolve(breadthArgument), report = read(path), prior = byId(read(resolve(priorBreadthArgument)));
  authenticate(path, report); assert.equal(report.results.length, 50);
  const execution = [], advisory = [], controlExecution = [], controlAdvisory = [], newCodes = [], errorDeliveryDifferences = [];
  for (const row of report.results) {
    const before = prior.get(row.id); assert(before); assert.equal(row.publishedTypingErrors.length, 0);
    assert.deepEqual(row.values, before.values); assert.deepEqual(row.errors.map(x => x.message), before.errors.map(x => x.message));
    const messages = values => [...new Set(values.map(error => error.message))].sort();
    assert.deepEqual(messages(row.pageErrors), messages(before.pageErrors));
    // Preserve changed transport delivery counts as a failed parity dimension,
    // rather than treating equivalent messages as full execution equivalence.
    if (row.pageErrors.length !== before.pageErrors.length) errorDeliveryDifferences.push({ id: row.id, before: before.pageErrors.length, after: row.pageErrors.length });
    if (row.runtime.every(runtime => runtime.version === '2.0.0-rc.9')) assert(row.attributionState.installed);
    else assert.equal(row.diagnosticSubscription, false);
    const events = row.feedback.filter(event => event.category === 'execution'), notes = row.feedback.filter(event => event.category === 'advisory');
    if (events.length || row.errors.length || row.pageErrors.length) (row.provenance.expectedIssue ? execution : controlExecution).push(row.id);
    if (notes.length) (row.provenance.expectedIssue ? advisory : controlAdvisory).push({ id: row.id, codes: notes.map(note => note.code) });
    const previousCodes = new Set(before.feedback.map(event => event.code));
    for (const event of row.feedback) if (!previousCodes.has(event.code)) newCodes.push({ id: row.id, code: event.code, category: event.category, severity: event.severity });
  }
  assert.equal(controlExecution.length, 0);
  breadthSummary = { executions: report.results.length, targetDirectRuntimeUnion: execution.length, targetAdvisories: advisory, controlExecution, controlAdvisory, newCodes,
    observedValueParity: true, errorMessageSetParity: true, errorDeliveryParity: errorDeliveryDifferences.length === 0, errorDeliveryDifferences };
}
const errorDeliveryStudies = [];
for (const argument of deliveryArguments) {
  const path = resolve(argument), report = read(path); authenticate(path, report, 'runtime-feedback.mjs');
  for (const row of report.results) {
    assert.equal(row.windowErrors.length, 1); assert.equal(row.windowErrors[0].errorId, 1);
    assert(row.pageErrors.length >= 1 && row.pageErrors.length <= 2);
    assert(row.pageErrors.every(error => error.message === row.windowErrors[0].message));
    errorDeliveryStudies.push({ path, id: row.id, windowEvents: row.windowErrors.length, playwrightDeliveries: row.pageErrors.length });
  }
}
const summary = { channelSpecimens: original.results.length, executedTwins: original.results.filter(row => !row.excludedBeforeExecution).length * 2,
  additionalExecutionTargets: channelOutcomes.filter(row => row.category === 'execution' && row.codes.length).length,
  advisoryTargets: channelOutcomes.filter(row => row.category === 'advisory' && row.codes.length).length,
  excludedTypeScriptTargets: channelOutcomes.filter(row => row.excludedTypeScriptCode).length,
  callbackOriginRecovered: true, identicalCodeIntentWitnesses: intentWitnesses.length,
  mechanismInventory: inventory.summary, breadthSummary, errorDeliveryStudies };
writeFileSync(output, JSON.stringify({ authority: false, summary, channelOutcomes, intentWitnesses, verifiedAt: new Date().toISOString() }, null, 2) + '\n');
console.log(JSON.stringify(summary, null, 2));
