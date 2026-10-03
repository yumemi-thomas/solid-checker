// Frozen evaluation of unchanged shared runtime channels, without source models.
// Expectations enter scoring only after the feedback function has returned.
import assert from 'node:assert/strict';
import { existsSync, readFileSync, realpathSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { closurePins, hash, packageRoot, read } from './catalog.mjs';
import { familyFeedback } from './family-feedback-system.mjs';
import { scoreHoldout, matchingClaims } from './family-holdout-score.mjs';
import { ts } from './lower.mjs';
import { oracleCompilerOptions } from '../../scripts/tsc-oracle.mjs';
const [populationArg, browserArg, outputArg] = process.argv.slice(2);
const populationPath = resolve(populationArg), browserPath = resolve(browserArg), output = resolve(outputArg);
const population = read(populationPath), browser = read(browserPath);
assert(!existsSync(output)); assert(browser.finishedAt); assert.equal(population.certification, false);
const cases = (await import(pathToFileURL(population.caseModule))).default;
const checked = new Map();
function pins(rows) { for (const pin of rows) {
  if (!checked.has(pin.path)) checked.set(pin.path, hash(readFileSync(pin.path)));
  assert.equal(checked.get(pin.path), pin.sha256, pin.path);
} }
pins([population.detector]); const detector = read(population.detector.path);
pins(detector.files); pins([detector.baseline]); pins(population.files); pins(population.declarations);
assert(new Date(detector.frozenAt) < new Date(population.frozenAt));
assert(new Date(population.frozenAt) < new Date(browser.startedAt));
const profile = dirname(dirname(browserPath)), before = read(join(profile, 'inputs-before.json')), after = read(join(profile, 'inputs-after.json'));
assert.deepEqual(before.files, after.files); pins(before.files);
for (const pkg of population.packages) assert.deepEqual(closurePins(pkg.root), pkg.pins);
assert.deepEqual(browser.results.map(row => row.id).sort(), population.rows.map(row => row.id).sort());
const results = [], typingPins = new Map();
for (const row of browser.results) {
  const expected = population.rows.find(item => item.id === row.id), item = cases.find(item => item.id === row.id);
  const root = join(dirname(browserPath), row.id), path = join(root, 'src/main.tsx'), text = readFileSync(path, 'utf8');
  assert.equal(hash(text), expected.sourceSha256); assert.equal(row.sourceSha256, expected.sourceSha256);
  assert.equal(row.sourceSha256, row.originalSourceSha256); assert.equal(hash(item.source), expected.sourceSha256);
  assert.equal(hash(item.flow.toString()), expected.flowSha256); assert.deepEqual(row.provenance, expected.provenance);
  assert.deepEqual(item.provenance, expected.provenance);
  for (const runtime of row.runtime) assert.equal(runtime.version, '2.0.0-rc.9');
  for (const pkg of row.packagePins) assert.deepEqual(pkg.pins, population.packages.find(p => p.package === pkg.package).pins);
  const options = ts.convertCompilerOptionsFromJson({ ...oracleCompilerOptions('v2', true, { customConditions: ['browser', 'development'] }), allowJs: true }, root).options;
  const program = ts.createProgram([path], options), errors = ts.getPreEmitDiagnostics(program).filter(d => d.category === ts.DiagnosticCategory.Error)
    .map(d => ({ code: d.code, message: ts.flattenDiagnosticMessageText(d.messageText, '\n') }));
  assert.deepEqual(errors, expected.publishedTypingErrors);
  assert.deepEqual(row.publishedTypingErrors.map(({code, message}) => ({code, message})), errors);
  for (const source of program.getSourceFiles().filter(source => source.isDeclarationFile)) {
    const real = realpathSync(source.fileName), digest = hash(readFileSync(real));
    assert(population.declarations.some(pin => pin.path === real && pin.sha256 === digest)); typingPins.set(real, digest);
  }
  // No id, package name, expectation, role or behavior is supplied to detection.
  const combined = familyFeedback({ publishedTypingErrors: errors, feedback: row.feedback, errors: row.errors,
    pageErrors: row.pageErrors, windowErrors: row.windowErrors });
  if (errors.length) { assert(row.excludedBeforeExecution); assert.equal(combined.feedback.length, 0); }
  const behavior = row.values?.behavior;
  if (!errors.length && !row.harnessFailure) assert(behavior, row.id);
  const evaluated = { id: row.id, package: item.package, provenance: expected.provenance, ...combined,
    publishedTypingErrors: errors, harnessFailure: row.harnessFailure ?? null,
    declaredBehavior: behavior ? {...behavior, passed: behavior.actual === behavior.desired} : null,
    callbackExecutions: row.values?.calls ?? 0, caughtErrors: row.values?.caught ?? [],
    diagnosticCodes: (row.observations ?? []).filter(note => note.channelCode).map(note => note.channelCode),
    reusedNativeAnalysis: false };
  const matched = matchingClaims(evaluated);
  const wanted = expected.provenance.expectedFeedbackSites;
  evaluated.feedbackSiteCheck = wanted === undefined || errors.length ? null : { wanted, actual: matched.length, passed: matched.length === wanted };
  results.push(evaluated);
}
const summary = scoreHoldout(results);
summary.callbackNotExecuted = results.filter(row => !row.excluded && row.callbackExecutions === 0 && row.provenance.callbackExpected !== false).map(row => row.id);
summary.siteCountFailures = results.filter(row => row.feedbackSiteCheck && !row.feedbackSiteCheck.passed).map(row => row.id);
const report = { authority: false, certification: false, finishedAt: new Date().toISOString(),
  basis: 'actual shared runtime diagnostics across original package callbacks; no inferred package contract',
  inputs: [populationPath, browserPath, join(profile, 'inputs-before.json'), join(profile, 'inputs-after.json')]
    .map(path => ({path, sha256: hash(readFileSync(path))})),
  evaluationInputs: [new URL(import.meta.url).pathname, new URL('./family-feedback-system.mjs', import.meta.url).pathname,
    new URL('./family-holdout-score.mjs', import.meta.url).pathname].map(path => ({path, sha256: hash(readFileSync(path))})),
  profileFrozenBeforePopulation: before.files.filter(pin => pin.path !== population.caseModule).every(pin => detector.files.some(d => d.path === pin.path && d.sha256 === pin.sha256)),
  validationTypings: [...typingPins].map(([path,sha256])=>({path,sha256})), summary, results };
pins(detector.files); pins(population.files); pins(population.declarations); pins(before.files);
writeFileSync(output, JSON.stringify(report, null, 2) + '\n'); console.log(JSON.stringify(summary, null, 2));
