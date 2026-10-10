// Reuse only byte-authenticated original analyses; recompute the new channels.
import assert from 'node:assert/strict';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { hash, read, closurePins, packageRoot } from './catalog.mjs';
import { ts } from './lower.mjs';
import { ClassFootprints } from './class-footprints.mjs';
import { classSnapshotFlowsV4, getterSnapshotsV7, snapshotFeedbackV7 } from './snapshot-feedback-v7.mjs';
import { scoreHoldout } from './family-holdout-score.mjs';
import { oracleCompilerOptions } from '../../scripts/tsc-oracle.mjs';
const [baselineArg, guardArg, outputArg, challengeArg] = process.argv.slice(2), started = performance.now(),
  baselinePath = resolve(baselineArg), baseline = read(baselinePath), output = resolve(outputArg);
assert(!existsSync(output)); assert.equal(baseline.authority, false); assert.equal(baseline.certification, false);
const files = new Set();
function walk(path) {
  path = resolve(path); if (files.has(path)) return; files.add(path); if (!path.endsWith('.mjs')) return;
  const source = ts.createSourceFile(path, readFileSync(path, 'utf8'), ts.ScriptTarget.Latest, true, ts.ScriptKind.JS);
  for (const node of source.statements) if ((ts.isImportDeclaration(node) || ts.isExportDeclaration(node)) && node.moduleSpecifier?.text?.startsWith('.')) walk(resolve(dirname(path), node.moduleSpecifier.text));
}
walk(new URL('./snapshot-refinement-study-v7.mjs', import.meta.url).pathname);
const frozen = [...files].map(path => ({ path, sha256: hash(readFileSync(path)) }));
const checked = new Map();
function checkPins(pins) { for (const pin of pins) { if (!checked.has(pin.path)) checked.set(pin.path, hash(readFileSync(pin.path)));
  assert.equal(checked.get(pin.path), pin.sha256, pin.path); } }
checkPins(baseline.inputs);
const browserPath = baseline.inputs.find(pin => pin.path.endsWith('/family-holdout-browser/browser/results.json')).path,
  staticPath = baseline.inputs.find(pin => pin.path.endsWith('/family-holdout-static/results.json')).path,
  populationPath = baseline.inputs.find(pin => pin.path.endsWith('/population.json')).path,
  browser = read(browserPath), statics = read(staticPath), population = read(populationPath), detector = read(population.detector.path);
checkPins(detector.files); checkPins(population.files); checkPins(population.declarations); checkPins(statics.inputs);
const guardPath = resolve(guardArg), guards = read(guardPath);
function authenticateBrowser(path, document) {
  assert(document.finishedAt); assert.equal(document.authority, false);
  const before = read(join(dirname(dirname(path)), 'inputs-before.json')), after = read(join(dirname(dirname(path)), 'inputs-after.json'));
  assert.deepEqual(before.files, after.files); checkPins(before.files);
  for (const row of document.results) {
    const root = join(dirname(path), row.id), main = join(root, 'src/main.tsx');
    assert.equal(hash(readFileSync(main)), row.sourceSha256); assert.equal(row.originalSourceSha256, row.sourceSha256);
    for (const pkg of row.packagePins) assert.deepEqual(closurePins(packageRoot(root, pkg.package)), pkg.pins);
  }
}
authenticateBrowser(browserPath, browser); authenticateBrowser(guardPath, guards);
const results = [], engine = new ClassFootprints();
const candidateCodes = ['SOURCE_CLASS_SNAPSHOT_FLOW', 'OBSERVED_GETTER_SNAPSHOT_FLOW', 'OBSERVED_PACKAGE_SNAPSHOT_FLOW'];
function evaluate(row, root, staticRow, guardRows) {
  const path = join(root, 'src/main.tsx'), text = readFileSync(path, 'utf8');
  if (row.publishedTypingErrors.length) return { ...baseline.results.find(item => item.id === row.id),
    id: row.id, package: row.packagePins[0].package, provenance: row.provenance,
    ...snapshotFeedbackV7({ publishedTypingErrors: row.publishedTypingErrors }), reusedNativeAnalysis: !!staticRow };
  const options = ts.convertCompilerOptionsFromJson({ ...oracleCompilerOptions('v2', true,
    { customConditions: ['browser', 'development'] }), allowJs: true }, root).options,
    program = ts.createProgram([path], options), source = program.getSourceFile(path);
  assert.equal(ts.getPreEmitDiagnostics(program).filter(error => error.category === ts.DiagnosticCategory.Error).length, 0);
  const classesV2 = classSnapshotFlowsV4(program, source, engine),
    observedGetters = getterSnapshotsV7(program, source, guardRows),
    observed = { publishedTypingErrors: row.publishedTypingErrors, feedback: row.feedback,
      errors: row.errors, pageErrors: row.pageErrors, windowErrors: row.windowErrors },
    combined = snapshotFeedbackV7(observed, { ...staticRow, originalPath: path, originalText: text, classesV2, observedGetters });
  const behavior = row.values?.behavior;
  return { id: row.id, package: row.packagePins[0].package, provenance: row.provenance, ...combined,
    harnessFailure: row.harnessFailure ?? null, declaredBehavior: behavior && { ...behavior, passed: behavior.actual === behavior.desired },
    classesV2, observedGetters, reusedNativeAnalysis: !!staticRow };
}
for (const row of browser.results) {
  const selected = population.rows.find(item => item.id === row.id), prior = baseline.results.find(item => item.id === row.id),
    staticRow = statics.results.find(item => item.id === row.id), root = join(dirname(browserPath), row.id),
    guardRow = guards.results.find(item => item.id === row.id);
  assert.deepEqual(row.provenance, selected.provenance); assert.deepEqual(prior.provenance, selected.provenance);
  assert.equal(row.sourceSha256, selected.sourceSha256); assert.equal(staticRow.sourceSha256, selected.sourceSha256);
  const remapped = (guardRow?.guardTrace ?? []).map(event => {
    assert.equal(guardRow.sourceSha256, row.sourceSha256); assert.deepEqual(guardRow.values, row.values);
    assert.deepEqual(guardRow.feedback.map(item => item.code), row.feedback.map(item => item.code));
    return { ...event, originalLocation: event.originalLocation && { ...event.originalLocation, path: join(root, 'src/main.tsx') } };
  });
  results.push(evaluate(row, root, staticRow, remapped));
}
function scoringRows(rows) {
  // This generic evaluation mapping recognizes informational snapshot claims.
  // The original labels stay untouched in the retained result and population.
  return rows.map(row => ({ ...row, provenance: { ...row.provenance,
    codes: [...row.provenance.codes ?? [], ...(row.provenance.rules?.includes('strict-read-untracked') ? candidateCodes : [])] } }));
}
const summary = scoreHoldout(scoringRows(results)), challenges = [];
if (challengeArg) {
  const path = resolve(challengeArg), challenge = read(path); authenticateBrowser(path, challenge);
  for (const row of challenge.results) challenges.push(evaluate(row, join(dirname(path), row.id), null, row.guardTrace));
}
for (const pin of frozen) assert.equal(hash(readFileSync(pin.path)), pin.sha256);
const report = { authority: false, certification: false, finishedAt: new Date().toISOString(), elapsedMs: performance.now() - started,
  inputs: [baselinePath, guardPath, ...(challengeArg ? [resolve(challengeArg)] : [])].map(path => ({ path, sha256: hash(readFileSync(path)) })),
  detectorInputs: frozen, originalLabelsPreserved: true, originalSourcesPreserved: true, adaptedAfterFreshChallengeObservation: true,
  evaluationMapping: { rule: 'strict-read-untracked', additionalInformationalCodes: candidateCodes,
    reason: 'Positive class footprints and observed getter guards identify setup snapshots; intent remains undeclared.' },
  summary, originalSnapshotHints: results.filter(row => row.provenance.role === 'target' && row.feedback.some(item => candidateCodes.includes(item.code))).map(row => row.id),
  challengeSummary: challengeArg ? scoreHoldout(scoringRows(challenges)) : null,
  results, challenges, limit: '18/18 means matching feedback including informational hints, not eighteen proven errors. Native analyses reused only for byte-identical original consumers and frozen inputs.' };
writeFileSync(output, JSON.stringify(report, null, 2) + '\n');
console.log(JSON.stringify({ original: { targets: summary.targets, matched: summary.matchedTargets, controls: summary.controls,
  quietCorrectControls: summary.quietCorrectControls, noisyControls: summary.noisyControls, controlBehaviorFailures: summary.controlBehaviorFailures,
  pairsPassed: summary.pairsPassed, typeExcluded: summary.typeExcluded }, hints: report.originalSnapshotHints,
  challenges: report.challengeSummary, elapsedMs: report.elapsedMs }, null, 2));
