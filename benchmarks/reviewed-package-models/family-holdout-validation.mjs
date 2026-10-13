// Authenticate frozen inputs, combine unchanged channels, then score claims.
import assert from 'node:assert/strict';
import { existsSync, readFileSync, realpathSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { closurePins, hash, packageDigest, packageRoot, read } from './catalog.mjs';
import { familyFeedback } from './family-feedback-system.mjs';
import { matchingClaims, scoreHoldout } from './family-holdout-score.mjs';
const [populationArg, browserArg, staticArg, outputArg, packageChecksArg] = process.argv.slice(2),
  populationPath = resolve(populationArg), browserPath = resolve(browserArg), staticPath = resolve(staticArg), output = resolve(outputArg);
assert(!existsSync(output));
const population = read(populationPath), browser = read(browserPath), statics = read(staticPath), frozen = read(population.detector.path);
assert.equal(population.authority, false); assert.equal(population.certification, false);
assert.equal(hash(readFileSync(population.detector.path)), population.detector.sha256);
assert(new Date(frozen.frozenAt) < new Date(population.frozenAt));
assert(new Date(population.frozenAt) < new Date(browser.startedAt));
assert(browser.finishedAt); assert(statics.finishedAt);
const pins = [...frozen.files, ...population.files, ...population.declarations, ...statics.inputs];
for (const pin of pins) assert.equal(hash(readFileSync(pin.path)), pin.sha256, pin.path);
for (const tool of population.tooling) assert.equal(packageDigest(tool.root), tool.digest, tool.package);
for (const pkg of frozen.packages) assert.deepEqual(closurePins(pkg.root), pkg.pins, pkg.package);
const before = read(join(dirname(dirname(browserPath)), 'inputs-before.json')),
  after = read(join(dirname(dirname(browserPath)), 'inputs-after.json'));
assert.deepEqual(before.files, after.files);
for (const pin of before.files) assert.equal(hash(readFileSync(pin.path)), pin.sha256);
assert.deepEqual(population.rows.map(row => row.id), browser.results.map(row => row.id));
assert.deepEqual(population.rows.map(row => row.id), statics.results.map(row => row.id));
const cases = (await import(pathToFileURL(new URL('./family-holdout-cases.mjs', import.meta.url).pathname))).default;
const results = [];
for (const row of browser.results) {
  const selected = population.rows.find(item => item.id === row.id), staticRow = statics.results.find(item => item.id === row.id),
    consumer = cases.find(item => item.id === row.id), root = join(dirname(browserPath), row.id), path = join(root, 'src/main.tsx'),
    text = readFileSync(path, 'utf8');
  assert.equal(hash(text), selected.sourceSha256); assert.equal(row.originalSourceSha256, selected.sourceSha256);
  assert.equal(row.sourceSha256, selected.sourceSha256); assert.equal(staticRow.sourceSha256, selected.sourceSha256);
  assert.equal(hash(consumer.source), selected.sourceSha256); assert.equal(hash(consumer.flow.toString()), selected.flowSha256);
  assert.deepEqual(row.provenance, selected.provenance); assert.deepEqual(consumer.provenance, selected.provenance);
  assert.deepEqual(row.publishedTypingErrors.map(error => error.code).sort(), selected.publishedTypingErrors.map(error => error.code).sort());
  for (const runtime of row.runtime) assert.equal(runtime.version, '2.0.0-rc.9');
  for (const pin of row.packagePins) assert.deepEqual(closurePins(packageRoot(root, pin.package)), pin.pins);
  if (selected.provenance.expectedTypingCode) {
    assert(row.excludedBeforeExecution); assert(row.publishedTypingErrors.some(error => error.code === selected.provenance.expectedTypingCode));
    assert.equal(staticRow.refused, 'TypeScript owns this input');
  } else assert.equal(row.publishedTypingErrors.length, 0, `Unexpected type-invalid consumer: ${row.id}`);
  // Detector receives observed facts only; scoring uses expectations afterward.
  const detected = familyFeedback({ publishedTypingErrors: row.publishedTypingErrors, feedback: row.feedback,
    errors: row.errors, pageErrors: row.pageErrors, windowErrors: row.windowErrors },
  { ...staticRow, originalPath: path, originalText: text });
  if (detected.excluded) { assert.equal(detected.feedback.length, 0); assert.equal(detected.gaps.length, 0); }
  const behavior = row.values?.behavior;
  const result = { id: row.id, package: selected.package, provenance: selected.provenance, ...detected,
    publishedTypingErrors: row.publishedTypingErrors, harnessFailure: row.harnessFailure ?? null,
    declaredBehavior: behavior ? { ...behavior, passed: behavior.actual === behavior.desired } : null,
    classCandidatesNotDisplayed: staticRow.classes?.candidates ?? [],
    callbackPhaseSites: staticRow.sites?.filter(site => site.extraction?.callbackPhase).map(site => ({ export: site.export, phase: site.extraction.callbackPhase })) ?? [],
    runtimePackageFrames: [] };
  const packageDir = packageRoot(root, selected.package);
  for (const item of row.feedback) for (const frame of item.originalFrames ?? []) {
    if (!frame?.path || !existsSync(frame.path)) continue;
    const real = realpathSync(frame.path); if (!real.startsWith(packageDir + '/')) continue;
    const bytes = readFileSync(real);
    result.runtimePackageFrames.push({ code: item.code, path: real, line: frame.line, column: frame.column,
      sourceSha256: hash(bytes), sourceLine: bytes.toString('utf8').split('\n')[frame.line - 1] });
  }
  result.matchedClaims = matchingClaims(result).map(item => ({ rule: item.rule, code: item.code, channel: item.channel, severity: item.severity, location: item.location }));
  results.push(result);
}
const summary = scoreHoldout(results);
const packageChecksPath = resolve(packageChecksArg), packageChecks = read(packageChecksPath);
assert(packageChecks.finishedAt); assert.equal(packageChecks.authority, false);
const checksBefore = read(join(dirname(dirname(packageChecksPath)), 'inputs-before.json')),
  checksAfter = read(join(dirname(dirname(packageChecksPath)), 'inputs-after.json'));
assert.deepEqual(checksBefore.files, checksAfter.files);
for (const pin of checksBefore.files) assert.equal(hash(readFileSync(pin.path)), pin.sha256);
const expectedCheckIds = ['holdout-check-package-same-length', 'holdout-check-package-different-length', 'holdout-check-native-same-length'];
assert.deepEqual(packageChecks.results.map(row => row.id), expectedCheckIds);
const checks = packageChecks.results.map((row, index) => {
  assert(!row.harnessFailure); assert.equal(row.publishedTypingErrors.length, 0);
  assert.equal(row.feedback.length, 0); assert.equal(row.errors.length, 0); assert.equal(row.pageErrors.length, 0);
  assert.equal(row.sourceSha256, row.originalSourceSha256);
  const root = join(dirname(packageChecksPath), row.id), source = join(root, 'src/main.tsx');
  assert.equal(hash(readFileSync(source)), row.sourceSha256);
  for (const pin of row.packagePins) assert.deepEqual(closurePins(packageRoot(root, pin.package)), pin.pins);
  assert.equal(row.values.behavior.actual, index === 2 ? '3,4' : '1,2');
  return { id: row.id, provenance: row.provenance, behavior: row.values.behavior, sourceSha256: row.sourceSha256 };
});
const byChannel = [...new Set(results.flatMap(row => row.feedback.map(item => item.channel)))].map(channel => ({ channel,
  matchedTargets: results.filter(row => row.provenance.role === 'target' && matchingClaims(row).some(item => item.channel === channel)).map(row => row.id),
  noisyControls: results.filter(row => row.provenance.role === 'control' && row.feedback.some(item => item.channel === channel)).map(row => row.id) }));
const report = { authority: false, certification: false, finishedAt: new Date().toISOString(),
  inputs: [populationPath, population.detector.path, browserPath, staticPath, packageChecksPath,
    new URL('./family-holdout-validation.mjs', import.meta.url).pathname, new URL('./family-holdout-score.mjs', import.meta.url).pathname]
    .map(path => ({ path, sha256: hash(readFileSync(path)) })),
  summary, byChannel, packageChecks: checks, scope: frozen.scope,
  limit: 'A deliberately authored heldout consumer population, not a random ecosystem sample or an all-package guarantee. Source candidates and assumptions remain distinct from proven violations. A rule/code match is required; unrelated feedback is a miss.', results };
writeFileSync(output, JSON.stringify(report, null, 2) + '\n'); console.log(JSON.stringify(summary, null, 2));
