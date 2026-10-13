// Authenticate retained observations, combine independent feedback channels,
// then compare with separately authored expectations. Misses remain visible.
import assert from 'node:assert/strict';
import { existsSync, mkdirSync, readFileSync, realpathSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { closurePins, hash, packageRoot, read } from './catalog.mjs';
import { familyFeedback } from './family-feedback-system.mjs';
import { ts } from './lower.mjs';
import { oracleCompilerOptions } from '../../scripts/tsc-oracle.mjs';
const [configArg, outputArg] = process.argv.slice(2), configPath = resolve(configArg), config = read(configPath), output = resolve(outputArg);
assert(!existsSync(output)); mkdirSync(output, { recursive: true });
assert.equal(config.authority, false); assert.equal(config.certification, false);
const inputPins = new Map(), typings = new Map(), authenticatedPackages = new Map();
function input(path) { path = resolve(path); const bytes = readFileSync(path), digest = hash(bytes);
  if (inputPins.has(path)) assert.equal(inputPins.get(path), digest); else inputPins.set(path, digest);
  return JSON.parse(bytes); }
input(configPath);
function checkPins(pins) { for (const pin of pins) assert.equal(hash(readFileSync(pin.path)), pin.sha256, `Input drift: ${pin.path}`); }
function browserDataset(path) {
  path = resolve(path); const browser = input(path); assert(browser.finishedAt); assert.equal(browser.authority, false);
  const before = input(join(dirname(dirname(path)), 'inputs-before.json')), after = input(join(dirname(dirname(path)), 'inputs-after.json'));
  assert.deepEqual(before.files, after.files); checkPins(before.files);
  return browser;
}
function authenticateRow(row, browserPath, allowHarnessFailure = false) {
  if (!allowHarnessFailure) assert(!row.harnessFailure, JSON.stringify(row.harnessFailure));
  const root = join(dirname(resolve(browserPath)), row.id), path = join(root, 'src/main.tsx'), text = readFileSync(path, 'utf8');
  assert.equal(hash(text), row.sourceSha256); assert.equal(row.sourceSha256, row.originalSourceSha256, 'Executed consumer was not rewritten');
  for (const runtime of row.runtime) assert.equal(runtime.version, '2.0.0-rc.9');
  for (const packagePin of row.packagePins) {
    const packageDir = packageRoot(root, packagePin.package), identity = JSON.stringify([packageDir, packagePin.pins]);
    if (!authenticatedPackages.has(identity)) { assert.deepEqual(closurePins(packageDir), packagePin.pins); authenticatedPackages.set(identity, packagePin.pins[0]); }
  }
  const options = ts.convertCompilerOptionsFromJson({ ...oracleCompilerOptions('v2', true, { customConditions: ['browser', 'development'] }), allowJs: true }, root).options;
  const program = ts.createProgram([path], options), diagnostics = ts.getPreEmitDiagnostics(program).filter(d => d.category === ts.DiagnosticCategory.Error);
  assert.deepEqual(diagnostics.map(d => d.code).sort(), row.publishedTypingErrors.map(d => d.code).sort(), 'Published typing verdict changed');
  for (const source of program.getSourceFiles().filter(source => source.isDeclarationFile)) {
    const real = realpathSync(source.fileName), digest = hash(readFileSync(real));
    if (typings.has(real)) assert.equal(typings.get(real), digest); else typings.set(real, digest);
  }
  return { root, path, text };
}
const results = [], rawPrecision = [];
for (const dataset of config.datasets) {
  const browser = browserDataset(dataset.browser), statics = input(dataset.static); assert(statics.finishedAt); checkPins(statics.inputs);
  const projection = dataset.projection ? input(dataset.projection) : null;
  if (projection) {
    assert.equal(resolve(projection.input.path), resolve(dataset.static)); assert.equal(hash(readFileSync(projection.input.path)), projection.input.sha256);
    assert.equal(hash(readFileSync(new URL('./family-project-warning.mjs', import.meta.url))), projection.projectorSha256);
  }
  assert.deepEqual(browser.results.map(r => r.id), statics.results.map(r => r.id));
  for (const row of browser.results) {
    const source = authenticateRow(row, dataset.browser), staticRow = statics.results.find(s => s.id === row.id);
    assert.equal(row.sourceSha256, staticRow.sourceSha256);
    const warnings = projection?.results.find(s => s.id === row.id)?.warnings ?? staticRow.warnings;
    // No provenance, expectations or declared behavior enter the detector.
    const observed = { publishedTypingErrors: row.publishedTypingErrors, feedback: row.feedback, errors: row.errors, pageErrors: row.pageErrors, windowErrors: row.windowErrors };
    const combined = familyFeedback(observed, { ...staticRow, originalPath: source.path, originalText: source.text }, warnings);
    const behavior = row.values?.behavior ?? null;
    results.push({ id: row.id, group: dataset.group, provenance: row.provenance, publishedTypingErrors: row.publishedTypingErrors,
      ...combined, declaredBehavior: behavior && { ...behavior, passed: behavior.actual === behavior.desired } });
  }
}
for (const path of config.precisionChecks) {
  const browser = browserDataset(path);
  for (const row of browser.results) { authenticateRow(row, path, true); rawPrecision.push({ id: row.id, provenance: row.provenance,
    harnessFailure: row.harnessFailure ?? null,
    feedback: familyFeedback({ publishedTypingErrors: row.publishedTypingErrors, feedback: row.feedback, errors: row.errors, pageErrors: row.pageErrors }).feedback,
    declaredBehavior: row.values?.behavior ?? null }); }
}
const valid = results.filter(r => !r.excluded), targets = valid.filter(r => r.provenance.role === 'target'), controls = valid.filter(r => r.provenance.role === 'control');
for (const row of targets) assert(controls.some(control => control.provenance.pair === row.provenance.pair), `Missing paired control: ${row.id}`);
for (const row of controls) if (row.declaredBehavior) assert(row.declaredBehavior.passed, `Correct-use behavior failed: ${row.id}`);
const misses = targets.filter(row => !row.feedback.length), noisyControls = controls.filter(row => row.feedback.length);
const byChannel = Object.fromEntries([...new Set(valid.flatMap(row => row.feedback.map(f => f.channel)))].sort()
  .map(channel => [channel, { targets: targets.filter(row => row.feedback.some(f => f.channel === channel)).length,
    controls: controls.filter(row => row.feedback.some(f => f.channel === channel)).length }]));
const summary = { records: results.length, executed: valid.length, typeExcluded: results.length - valid.length,
  targetControlPairs: targets.length, targetsWithFeedback: targets.length - misses.length, automaticMisses: misses.map(r => r.id),
  controls: controls.length, quietControls: controls.length - noisyControls.length, noisyControls: noisyControls.map(r => r.id),
  precisionRecords: rawPrecision.length, precisionHarnessFailures: rawPrecision.filter(r => r.harnessFailure).length,
  declaredBehaviorFailures: targets.filter(r => r.declaredBehavior && !r.declaredBehavior.passed).map(r => r.id), byChannel };
const rulesPath = resolve('packages/cli/lib/rules-solid-v2.json'), manifest = input(rulesPath);
const expectedRules = new Set(targets.flatMap(r => r.provenance.rules));
const ruleInventory = manifest.rules.map(rule => ({ ...rule, exercisedConsumerClaim: expectedRules.has(rule.name),
  receivedNativeStaticViolation: targets.some(row => row.feedback.some(f => f.channel === 'native-static' && f.rule === rule.name)),
  receivedSourceAssumption: targets.some(row => row.feedback.some(f => f.channel === 'source-assumption' && f.rule === rule.name)),
  gap: rule.uncertifiable ? 'gap or environment diagnostic; no misuse claim' : !expectedRules.has(rule.name) ? 'not exercised in this study' :
    misses.some(row => row.provenance.rules.includes(rule.name)) ? 'exercised silent miss' : 'finite consumer feedback; full rule coverage unproven' }));
for (const [path, sha256] of inputPins) assert.equal(hash(readFileSync(path)), sha256);
for (const [path, sha256] of typings) assert.equal(hash(readFileSync(path)), sha256);
const report = { authority: false, certification: false, finishedAt: new Date().toISOString(), summary,
  inputs: [...inputPins].map(([path, sha256]) => ({ path, sha256 })),
  validationTypings: [...typings].map(([path, sha256]) => ({ path, sha256 })),
  typingPinLimit: 'Declaration digests captured during this validation, not retroactively before the browser runs.',
  systemInputs: ['family-feedback-study.mjs', 'family-feedback-system.mjs'].map(name => { const path = new URL(name, import.meta.url).pathname; return { path, sha256: hash(readFileSync(path)) }; }),
  results, precisionChecks: rawPrecision, ruleInventory };
writeFileSync(join(output, 'results.json'), JSON.stringify(report, null, 2) + '\n'); console.log(JSON.stringify(summary, null, 2));
