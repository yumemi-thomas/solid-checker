import assert from 'node:assert/strict';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { closurePins, hash, packageRoot, read } from './catalog.mjs';
import cases from './class-footprint-cases.mjs';
const [studyArgument, inventoryArgument, originalArgument, tracedArgument, outputArgument] = process.argv.slice(2);
const paths = [studyArgument, inventoryArgument, originalArgument, tracedArgument].map(path => resolve(path)), output = resolve(outputArgument); assert(!existsSync(output));
const [study, inventory, original, traced] = paths.map(read);
for (const input of study.frozen) assert.equal(hash(readFileSync(new URL(input.name, import.meta.url))), input.sha256);
assert.equal(inventory.extractorSha256, hash(readFileSync(new URL('./class-footprints.mjs', import.meta.url))));
assert.equal(original.results.length, cases.length); assert.equal(traced.results.length, cases.length);
const outcomes = [];
for (const specimen of cases) {
  const left = original.results.find(row => row.id === specimen.id), right = traced.results.find(row => row.id === specimen.id), staticRow = study.results.find(row => row.id === specimen.id);
  assert(left && right && staticRow); assert.equal(staticRow.refused, null, specimen.id);
  for (const [row, browserPath] of [[left, paths[2]], [right, paths[3]]]) {
    assert.equal(row.sourceSha256, hash(specimen.source)); assert.deepEqual(row.publishedTypingErrors, []);
    assert.equal(row.executionError, undefined); assert.deepEqual(row.blockedRequests, []); assert.deepEqual(row.pageErrors, []);
    const root = join(dirname(browserPath), row.id);
    for (const input of row.packagePins) assert.deepEqual(closurePins(packageRoot(root, input.package)), input.pins);
    assert.equal(hash(readFileSync(join(root, 'src/main.tsx'))), row.sourceSha256);
    assert.deepEqual(row.feedback, []);
    for (const event of row.guardTrace ?? []) {
      assert.equal(event.severity, 'info'); assert.equal(event.certification, false);
      assert.equal(hash(readFileSync(event.path)), event.sourceSha256);
      assert(event.originalLocation?.path.startsWith(root + '/src/'));
    }
  }
  assert.deepEqual(left.values, right.values); assert.deepEqual(left.errors, right.errors);
  assert.deepEqual(left.windowErrors, right.windowErrors);
  assert.equal(right.values.behavior.actual !== right.values.behavior.desired, specimen.provenance.expectedIssue);
  assert.equal(staticRow.candidates.length > 0, specimen.provenance.expectedCandidate);
  for (const model of staticRow.models) for (const input of model.sources) assert.equal(hash(readFileSync(input.path)), input.sha256);
  for (const candidate of staticRow.candidates) {
    assert.equal(candidate.severity, 'info'); assert.equal(candidate.certification, false);
    for (const premise of candidate.premises) assert.equal(hash(readFileSync(premise.path)), premise.sourceSha256);
  }
  outcomes.push({ id: specimen.id, role: specimen.provenance.role, behavioralFailure: specimen.provenance.expectedIssue,
    sourceCandidate: staticRow.candidates.length > 0, guardKinds: [...new Set((right.guardTrace ?? []).map(event => event.kind))],
    openUses: staticRow.openUses, gaps: specimen.provenance.gap ?? null });
}
const intentional = cases.find(row => row.id === 'class-implicit-snapshot-control'), incorrect = cases.find(row => row.id === 'class-map-get-target');
assert.equal(intentional.source, incorrect.source); // Intent alone changes the expected result.
const times = study.results.filter(row => row.id.startsWith('class-')).map(row => row.analysisMs).sort((a, b) => a - b);
const summary = { ...study.summary, browserExecutions: cases.length * 2, publishedTypingErrors: 0, finiteBehaviorParity: true,
  newTargetsWithGuardNotes: outcomes.filter(row => row.behavioralFailure && row.guardKinds.length).length,
  validControlsWithGuardNotes: outcomes.filter(row => !row.behavioralFailure && row.guardKinds.length).map(row => row.id),
  implicitIntentPairCodeIdentical: true, sourceAnalysisMs: { median: times[Math.floor(times.length / 2)], p90: times[Math.floor(times.length * 0.9)] },
  inventory: inventory.summary, automaticProvenViolationsAdded: 0 };
writeFileSync(output, JSON.stringify({ authority: false, certification: false, inputs: paths.map(path => ({ path, sha256: hash(readFileSync(path)) })),
  summary, outcomes, verifiedAt: new Date().toISOString() }, null, 2) + '\n'); console.log(JSON.stringify(summary, null, 2));
