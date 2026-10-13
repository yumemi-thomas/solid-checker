// Validate paired outcomes, retain misses, and compare instrumentation with
// actual package execution. Counts distinguish bug findings from guard notes.
import assert from "node:assert/strict";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import cases from "./breadth-cases.mjs";
import { closurePins, hash, packageRoot, read } from "./catalog.mjs";
import { instrumentGuards } from "./guard-trace.mjs";
const [outputArgument, rawArgument, tracedArgument, staticArgument] = process.argv.slice(2);
const output = resolve(outputArgument); assert(!existsSync(output));
const study = read(process.env.REVIEWED_MODEL_BREADTH_STUDY ?? 'rust/target/reviewed-models-breadth-study.json');
assert.equal(hash(readFileSync(new URL('./breadth-cases.mjs', import.meta.url))), study.corpusSha256);
for (const input of study.frozen) assert.equal(hash(readFileSync(new URL(input.name, import.meta.url))), input.digest);
const rawPath = resolve(rawArgument), tracedPath = resolve(tracedArgument), staticPath = resolve(staticArgument);
const raw = read(rawPath), traced = read(tracedPath), statics = read(staticPath);
assert(raw.finishedAt && traced.finishedAt && statics.finishedAt); assert.equal(raw.results.length, cases.length); assert.equal(traced.results.length, cases.length);
assert.equal(statics.browserSha256, hash(readFileSync(rawPath)));
const byId = report => new Map(report.results.map(row => [row.id, row]));
const rawRows = byId(raw), traceRows = byId(traced), staticRows = byId(statics);
let locations = 0, guardLocations = 0;
for (const [path, report] of [[rawPath, raw], [tracedPath, traced]]) for (const row of report.results) {
  const sourcePath = join(dirname(path), row.id, row.mountResultExposedForDisposal ? 'src/index.tsx' : 'src/main.tsx');
  assert.equal(hash(readFileSync(sourcePath)), row.sourceSha256); assert.equal(row.publishedTypingErrors.length, 0); assert(!row.excludedBeforeExecution); assert(!row.harnessFailure);
  for (const pkg of row.packagePins) assert.deepEqual(closurePins(packageRoot(dirname(sourcePath), pkg.package)), pkg.pins);
  if (row.patchEvidence) assert.equal(hash(readFileSync(row.patchEvidence.path)), row.patchEvidence.mutatedSha256);
  for (const event of row.feedback) { assert.equal(event.certification, false); assert(event.originalLocation); assert(existsSync(event.originalLocation.path)); locations++; }
  for (const event of row.guardTrace ?? []) {
    assert.equal(event.severity, 'info'); assert.equal(event.certification, false); assert.equal(event.sourceSha256, hash(readFileSync(event.path)));
    const reconstructed = instrumentGuards(readFileSync(event.path, 'utf8'), event.path); assert(reconstructed.observations.some(site => site.kind === event.kind && site.start === event.start && site.end === event.end));
    assert(event.originalLocation); assert(existsSync(event.originalLocation.path)); guardLocations++;
  }
}
for (const row of traced.results) {
  assert.equal(row.guardInstrumentation.instrumenterSha256, hash(readFileSync(new URL('./guard-trace.mjs', import.meta.url))));
  assert.equal(row.guardInstrumentation.collectorSha256, hash(readFileSync(new URL('./guard-trace-runtime.mjs', import.meta.url))));
  const prior = rawRows.get(row.id); assert.deepEqual(row.values, prior.values, `Instrumentation changed observed values: ${row.id}`);
  assert.deepEqual(row.feedback.map(f => f.code), prior.feedback.map(f => f.code), `Instrumentation changed semantic observations: ${row.id}`);
  assert.deepEqual(row.errors.map(e => e.message), prior.errors.map(e => e.message)); assert.deepEqual(row.pageErrors.map(e => e.message), prior.pageErrors.map(e => e.message));
}
function findingKey(finding, root) {
  const location = finding.primaryLocation ?? finding.location;
  return JSON.stringify([finding.rule, finding.message, relative(root, location.path),
    readFileSync(location.path).subarray(location.startByte, location.endByte).toString('utf8')]);
}
const outcomes = [];
for (const entry of cases) {
  const row = rawRows.get(entry.id), tracedRow = traceRows.get(entry.id), staticRow = staticRows.get(entry.id), expectedIssue = entry.provenance.expectedIssue;
  const behavioralFailure = row.values.behavior ? row.values.behavior.desired !== row.values.behavior.actual : false;
  const diagnostic = row.feedback.length > 0, exception = row.errors.length > 0 || row.pageErrors.length > 0;
  if (!expectedIssue) { assert(!diagnostic && !exception && !behavioralFailure, `Valid control failed: ${entry.id}`); }
  else assert(diagnostic || exception || behavioralFailure, `Mutation survived every observation: ${entry.id}`);
  let introducedNative = [], introducedModel = [];
  const rawRoot = join(dirname(rawPath), row.id);
  if (!staticRow.refused && expectedIssue) {
    const control = staticRows.get(row.id.replace(/-target$/, '-control')), controlRoot = join(dirname(rawPath), control.id);
    const existing = new Set(control.baseline.findings.filter(f => f.kind === 'violation').map(f => findingKey(f, controlRoot)));
    introducedNative = staticRow.baselineRelevantViolations.filter(f => !existing.has(findingKey(f, rawRoot)));
    const controlModel = new Set(control.warnings.map(f => findingKey(f, controlRoot)));
    introducedModel = staticRow.modelRelevantWarnings.filter(f => !controlModel.has(findingKey(f, rawRoot)));
  }
  for (const input of staticRow.sources) assert.equal(hash(readFileSync(input.path)), input.sha256);
  outcomes.push({ id: row.id, pair: row.provenance.pair, role: row.provenance.role, family: row.provenance.family,
    expectedIssue, diagnostic, exception, behavioralFailure, guardKinds: tracedRow.guardTrace.map(event => event.kind),
    staticRefused: staticRow.refused, introducedNativeRules: introducedNative.map(f => f.rule), introducedModelRules: introducedModel.map(f => f.rule),
    existingNativeControlRules: !expectedIssue ? staticRow.baseline?.findings.filter(f => f.kind === 'violation').map(f => f.rule) ?? [] : [] });
}
const targets = outcomes.filter(row => row.expectedIssue), controls = outcomes.filter(row => !row.expectedIssue);
const summary = { targets: targets.length, controls: controls.length, directRuntimeDiagnostics: targets.filter(row => row.diagnostic).length,
  runtimeExceptions: targets.filter(row => row.exception).length,
  directRuntimeUnion: targets.filter(row => row.diagnostic || row.exception).length,
  quietTargetsWithNewGuardNotes: targets.filter(row => !row.diagnostic && !row.exception && row.guardKinds.length).length,
  targetsWithoutRuntimeFindingOrGuard: targets.filter(row => !row.diagnostic && !row.exception && !row.guardKinds.length).map(row => row.id),
  behavioralFailures: targets.filter(row => row.behavioralFailure).length,
  combinedRuntimeAndBehavior: targets.filter(row => row.diagnostic || row.exception || row.behavioralFailure).length,
  controlRuntimeOrBehaviorFailures: controls.filter(row => row.diagnostic || row.exception || row.behavioralFailure).length,
  controlsWithInformationalGuardNotes: controls.filter(row => row.guardKinds.length).map(row => row.id),
  staticEligibleTargets: targets.filter(row => !row.staticRefused).length,
  introducedNativeDetections: targets.filter(row => row.introducedNativeRules.length).length,
  introducedSourceModelDetections: targets.filter(row => row.introducedModelRules.length).length,
  combinedStaticDetections: targets.filter(row => row.introducedNativeRules.length || row.introducedModelRules.length).length,
  controlsWithExistingNativeFindings: controls.filter(row => row.existingNativeControlRules.length).map(row => row.id),
  originalLocationsBothBrowserRuns: locations, originalGuardLocations: guardLocations,
  nativeAnalyses: statics.results.filter(row => !row.refused).length * 2 };
assert.equal(summary.targets, 24); assert.equal(summary.controls, 26); assert.equal(summary.directRuntimeUnion, 15); assert.equal(summary.quietTargetsWithNewGuardNotes, 8);
assert.equal(summary.combinedRuntimeAndBehavior, 24); assert.equal(summary.controlRuntimeOrBehaviorFailures, 0);
assert.equal(summary.combinedStaticDetections, 4);
writeFileSync(output, JSON.stringify({ authority: false, study: 'paired authored mutation study, not independent population precision', rawPath, tracedPath, staticPath,
  summary, outcomes, verifiedAt: new Date().toISOString() }, null, 2) + '\n'); console.log(JSON.stringify(summary, null, 2));
