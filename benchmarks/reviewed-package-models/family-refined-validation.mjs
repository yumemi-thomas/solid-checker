// Scoring and expectations live here, after independently produced feedback.
// The comparison refuses removed, relabeled or silently rewritten consumers.
import assert from 'node:assert/strict';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { hash, read } from './catalog.mjs';
const [oldArg, nextArg, challengeBrowserArg, challengeStaticArg, outputArg, originalIntentArg] = process.argv.slice(2);
const oldPath = resolve(oldArg), nextPath = resolve(nextArg), output = resolve(outputArg), old = read(oldPath), next = read(nextPath);
assert(!existsSync(output));
for (const report of [old, next]) {
  assert.equal(report.authority, false); assert.equal(report.certification, false); assert(report.finishedAt);
  for (const pin of [...report.inputs, ...report.systemInputs, ...report.validationTypings]) assert.equal(hash(readFileSync(pin.path)), pin.sha256);
}
function rows(report) {
  const config = report.inputs.filter(pin => pin.path.endsWith('.json')).map(pin => ({ pin, document: read(pin.path) })).find(entry => entry.document.datasets)?.document;
  assert(config);
  return config.datasets.flatMap(dataset => read(resolve(dataset.browser)).results.map(row => ({ ...row,
    sourcePath: join(dirname(resolve(dataset.browser)), row.id, 'src/main.tsx') })));
}
const before = rows(old), after = rows(next);
assert.deepEqual(after.map(r => r.id).sort(), before.map(r => r.id).sort());
const sourceChanges = [];
for (const row of after) {
  const prior = before.find(r => r.id === row.id); assert.equal(hash(readFileSync(row.sourcePath)), row.sourceSha256);
  assert.equal(hash(readFileSync(prior.sourcePath)), prior.sourceSha256);
  if (prior.sourceSha256 !== row.sourceSha256) sourceChanges.push({ id: row.id, beforeSha256: prior.sourceSha256, afterSha256: row.sourceSha256 });
  for (const key of ['role', 'pair', 'family', 'expectedIssue', 'rules', 'codes', 'expectedTypingCode', 'expectedGap'])
    assert.deepEqual(row.provenance[key], prior.provenance[key], `Consumer was relabeled: ${row.id}.${key}`);
}
assert.deepEqual(sourceChanges.map(row => row.id), ['family-intentional-snapshot-control']);
const snapshotBefore = before.find(r => r.id === sourceChanges[0].id), snapshotAfter = after.find(r => r.id === sourceChanges[0].id);
const explicit = `import { untrack as explicitSnapshot } from 'solid-js';\n` +
  readFileSync(snapshotBefore.sourcePath, 'utf8').replace('const frozen = value();', 'const frozen = explicitSnapshot(value);');
assert.equal(readFileSync(snapshotAfter.sourcePath, 'utf8'), explicit);
assert.deepEqual(snapshotAfter.values.behavior, snapshotBefore.values.behavior);
const runtimeRules = {
  'strict-read-untracked': 'STRICT_READ_UNTRACKED',
  'reactive-write-in-owned-scope': 'REACTIVE_WRITE_IN_OWNED_SCOPE',
  'action-called-in-owned-scope': 'ACTION_CALLED_IN_OWNED_SCOPE',
  'leaf-owner-forbidden-call': 'CLEANUP_IN_FORBIDDEN_SCOPE',
  'missing-owner': 'NO_OWNER_CLEANUP',
  'pending-async-unsuspendable-read': 'PENDING_ASYNC_UNTRACKED_READ',
  'async-outside-loading-boundary': 'ASYNC_OUTSIDE_LOADING_BOUNDARY',
};
const valid = next.results.filter(row => !row.excluded), targets = valid.filter(row => row.provenance.role === 'target'), controls = valid.filter(row => row.provenance.role === 'control');
const matches = targets.map(row => {
  const rules = row.provenance.rules ?? [], codes = row.provenance.codes ?? [];
  const feedback = row.feedback.filter(item => rules.includes(item.rule) || codes.includes(item.code) ||
    rules.some(rule => runtimeRules[rule] && runtimeRules[rule] === item.code) ||
    rules.includes('strict-read-untracked') && item.code === 'SOURCE_GETTER_SNAPSHOT_FLOW' ||
    rules.includes('no-direct-mutation') && item.code === 'SOURCE_GETTER_WRITE_CANDIDATE' ||
    rules.includes('flush-in-action') && item.channel === 'runtime-exception' && item.message.startsWith('[FLUSH_IN_ACTION]'));
  assert(feedback.length, `Only unrelated feedback or no feedback: ${row.id}`);
  return { id: row.id, expectedRules: rules, expectedCodes: codes,
    matched: feedback.map(item => ({ rule: item.rule, code: item.code, channel: item.channel, severity: item.severity })) };
});
for (const row of controls) assert.equal(row.feedback.length, 0, `Noisy control: ${row.id}`);
assert.equal(valid.length, 45); assert.equal(targets.length, 21); assert.equal(controls.length, 24);
assert.equal(next.results.filter(row => row.excluded).length, 1);
assert.equal(next.results.find(row => row.excluded).feedback.length, 0);
const originalIntentPath = resolve(originalIntentArg), originalIntent = read(originalIntentPath);
assert(originalIntent.finishedAt); for (const pin of originalIntent.inputs) assert.equal(hash(readFileSync(pin.path)), pin.sha256);
assert.equal(originalIntent.results.length, 1); assert.equal(originalIntent.results[0].id, snapshotBefore.id);
assert.equal(originalIntent.results[0].sourceSha256, snapshotBefore.sourceSha256);
assert(originalIntent.results[0].warnings.some(w => w.rule === 'strict-read-untracked'));
const browserPath = resolve(challengeBrowserArg), staticPath = resolve(challengeStaticArg), browser = read(browserPath), statics = read(staticPath);
assert(browser.finishedAt); assert(statics.finishedAt);
for (const pin of statics.inputs) assert.equal(hash(readFileSync(pin.path)), pin.sha256);
const beforeInputs = read(join(dirname(dirname(browserPath)), 'inputs-before.json')), afterInputs = read(join(dirname(dirname(browserPath)), 'inputs-after.json'));
assert.deepEqual(beforeInputs.files, afterInputs.files); for (const pin of beforeInputs.files) assert.equal(hash(readFileSync(pin.path)), pin.sha256);
assert.deepEqual(browser.results.map(r => r.id), statics.results.map(r => r.id));
const challenges = browser.results.map(row => {
  assert(!row.harnessFailure); assert.equal(row.publishedTypingErrors.length, 0);
  const staticRow = statics.results.find(r => r.id === row.id); assert.equal(staticRow.sourceSha256, row.sourceSha256);
  assert.equal(hash(readFileSync(join(dirname(browserPath), row.id, 'src/main.tsx'))), row.sourceSha256);
  for (const model of staticRow.catalog.models) assert.deepEqual(model.pins, row.packagePins.find(pin => pin.package === model.package)?.pins);
  const behavior = row.values.behavior, warnings = staticRow.warnings;
  if (row.provenance.role === 'control') {
    assert.equal(warnings.length, 0); assert.equal(row.feedback.length, 0);
    assert.equal(row.errors.length, 0); assert.equal(row.pageErrors.length, 0); assert.equal(behavior.actual, behavior.desired);
  } else if (row.provenance.expectedGap) {
    assert.equal(warnings.length, 0); assert.notEqual(behavior.actual, behavior.desired);
  } else {
    assert(warnings.some(w => w.rule === 'reactive-read-after-await'));
    assert.notEqual(behavior.actual, behavior.desired);
  }
  return { id: row.id, role: row.provenance.role, open: row.provenance.expectedGap, warnings: warnings.map(w => w.rule), behavior };
});
const report = { authority: false, certification: false, finishedAt: new Date().toISOString(),
  inputs: [oldPath, nextPath, browserPath, staticPath, originalIntentPath, new URL('./family-refined-validation.mjs', import.meta.url).pathname]
    .map(path => ({ path, sha256: hash(readFileSync(path)) })),
  score: { correctlyHandled: matches.length + controls.length, total: valid.length, matchedTargets: matches.length, quietControls: controls.length,
    typingExclusions: 1, sourceEdits: sourceChanges.length, challengeWarnings: challenges.filter(r => r.warnings.length).length,
    challengeQuietControls: challenges.filter(r => r.role === 'control').length, challengeOpen: challenges.filter(r => r.open).map(r => r.id) },
  limit: '45/45 on the revised corpus with one explicit snapshot intent edit; source candidates, preferences and declared lifetimes still count as feedback.',
  originalUnchangedCorpus: { correctlyHandled: 44, total: 45, remaining: snapshotBefore.id,
    reason: 'Existing snapshot has no source-level intent signal and still receives a strict-read warning.' },
  sourceChanges, matches, challenges };
writeFileSync(output, JSON.stringify(report, null, 2) + '\n'); console.log(JSON.stringify(report.score, null, 2));
