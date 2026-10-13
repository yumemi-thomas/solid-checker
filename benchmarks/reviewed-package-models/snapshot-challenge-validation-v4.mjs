// Validate retained observations against the population sealed before execution.
import assert from 'node:assert/strict';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { hash, read, closurePins } from './catalog.mjs';
import { scoreHoldout } from './family-holdout-score.mjs';
const [populationArg, browserArg, studyArg, outputArg, legacyCasesArg] = process.argv.slice(2), output = resolve(outputArg),
  populationPath = resolve(populationArg), browserPath = resolve(browserArg), studyPath = resolve(studyArg),
  population = read(populationPath), browser = read(browserPath), study = read(studyPath);
const casePath = population.caseModule ?? resolve(legacyCasesArg);
assert(population.files.some(pin => pin.path === casePath));
const cases = (await import(pathToFileURL(casePath))).default;
assert(!existsSync(output)); assert.equal(population.authority, false); assert.equal(study.certification, false);
assert(browser.finishedAt); assert.equal(browser.results.length, population.rows.length);
assert.deepEqual(study.challenges.map(row => row.id).sort(), population.rows.map(row => row.id).sort());
assert.deepEqual(cases.map(row => row.id).sort(), population.rows.map(row => row.id).sort());
assert(study.inputs.some(pin => pin.path === browserPath && pin.sha256 === hash(readFileSync(browserPath))));
const checked = new Map();
function pins(rows) { for (const row of rows) { if (!checked.has(row.path)) checked.set(row.path, hash(readFileSync(row.path)));
  assert.equal(checked.get(row.path), row.sha256, row.path); } }
pins([population.detector]); const detector = read(population.detector.path); pins(detector.files); pins([detector.baseline]);
assert(new Date(detector.frozenAt) < new Date(population.frozenAt));
assert(new Date(population.frozenAt) < new Date(browser.startedAt));
pins(population.files); pins(population.declarations); pins(study.inputs); pins(study.detectorInputs);
const detectorFrozenBeforeThisPopulation = study.detectorInputs.every(pin => detector.files.some(frozen =>
  frozen.path === pin.path && frozen.sha256 === pin.sha256));
const scoring = study.challenges.map(row => ({ ...row, provenance: { ...row.provenance,
  codes: [...row.provenance.codes ?? [], ...(row.provenance.rules?.includes(study.evaluationMapping.rule)
    ? study.evaluationMapping.additionalInformationalCodes : [])] } }));
assert.deepEqual(study.challengeSummary, scoreHoldout(scoring));
const profile = dirname(dirname(browserPath)), before = read(join(profile, 'inputs-before.json')), after = read(join(profile, 'inputs-after.json'));
assert.deepEqual(before.files, after.files); pins(before.files);
for (const pkg of population.packages) assert.deepEqual(closurePins(pkg.root), pkg.pins);
for (const selected of population.rows) {
  const item = cases.find(row => row.id === selected.id), observed = browser.results.find(row => row.id === selected.id),
    feedback = study.challenges.find(row => row.id === selected.id), path = join(dirname(browserPath), selected.id, 'src/main.tsx'), text = readFileSync(path, 'utf8');
  assert(item && observed && feedback); assert.equal(hash(item.source), selected.sourceSha256);
  assert.equal(hash(item.flow.toString()), selected.flowSha256); assert.equal(hash(text), selected.sourceSha256);
  assert.deepEqual(item.provenance, selected.provenance); assert.deepEqual(observed.provenance, selected.provenance);
  assert.deepEqual(feedback.provenance, selected.provenance); assert.equal(observed.sourceSha256, selected.sourceSha256);
  assert.deepEqual(observed.publishedTypingErrors.map(({ code, message }) => ({ code, message })), selected.publishedTypingErrors);
  assert.equal(feedback.reusedNativeAnalysis, false);
  if (selected.publishedTypingErrors.length) {
    assert(feedback.excluded); assert.equal(feedback.feedback.length, 0);
    assert(observed.excludedBeforeExecution); assert(selected.publishedTypingErrors.some(error => error.code === selected.provenance.expectedTypingCode));
  } else {
    assert.equal(observed.harnessFailure ?? null, null); assert.equal(feedback.harnessFailure, null);
    assert.deepEqual(feedback.declaredBehavior, { ...observed.values.behavior,
      passed: observed.values.behavior.actual === observed.values.behavior.desired });
    for (const hint of feedback.feedback.filter(note => note.channel === 'source-candidate')) {
      assert.equal(hint.certification, false); assert.equal(hint.severity, 'info'); assert.equal(hint.category, 'intent-open');
      assert(hint.start >= 0 && hint.end > hint.start && hint.end <= text.length);
      assert.equal(hint.location.startByte, Buffer.byteLength(text.slice(0, hint.start)));
      assert.equal(hint.location.path, path);
    }
  }
}
const result = { authority: false, certification: false, finishedAt: new Date().toISOString(),
  inputs: [populationPath, browserPath, studyPath].map(path => ({ path, sha256: hash(readFileSync(path)) })),
  validator: { path: new URL(import.meta.url).pathname, sha256: hash(readFileSync(new URL(import.meta.url))) },
  consumers: population.rows.length, publishedDeclarations: population.declarations.length,
  checks: ['population frozen before browser execution', 'detector membership checked against population freeze', 'independent scoring', 'original consumers, flows and labels', 'published types and typing exclusions',
    'package closure bytes', 'before/after browser profile', 'feedback span provenance', 'no native-analysis reuse for fresh consumers'],
  detectorFrozenBeforeThisPopulation, adaptedAfterThisPopulation: !detectorFrozenBeforeThisPopulation, adaptedAfterInitialChallenges: !!study.adaptedAfterFreshChallengeObservation,
  summary: study.challengeSummary };
writeFileSync(output, JSON.stringify(result, null, 2) + '\n'); console.log(JSON.stringify({ consumers: result.consumers,
  declarations: result.publishedDeclarations, detectorFrozenBeforePopulation: result.detectorFrozenBeforeThisPopulation, targets: result.summary.matchedTargets,
  controlsQuiet: result.summary.quietCorrectControls, noisyControls: result.summary.noisyControls }));
