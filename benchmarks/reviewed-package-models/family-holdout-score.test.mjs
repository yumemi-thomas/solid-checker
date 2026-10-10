import assert from 'node:assert/strict';
import test from 'node:test';
import { familyFeedback } from './family-feedback-system.mjs';
import { scoreHoldout } from './family-holdout-score.mjs';
const row = (id, role, feedback, extra = {}) => ({ id, package: 'test', provenance: { pair: 'one', role,
  rules: ['reactive-read-after-await'], codes: [] }, feedback, ...extra });
test('unrelated warnings cannot improve the score; noisy paired controls fail the pair', () => {
  const observed = scoreHoldout([row('bad', 'target', [{ rule: 'strict-read-untracked' }]),
    row('good', 'control', [{ rule: 'reactive-read-after-await' }])]);
  assert.equal(observed.correctlyHandled, 0); assert.equal(observed.pairsPassed, 0);
  assert.deepEqual(observed.unrelatedFeedbackTargets, ['bad']); assert.deepEqual(observed.noisyControls, ['good']);
});
test('harness failures and wrong control behavior cannot pass as quiet controls', () => {
  const observed = scoreHoldout([row('bad', 'target', [{ rule: 'reactive-read-after-await' }]),
    row('good', 'control', [], { declaredBehavior: { passed: false } }),
    row('broken', 'control', [], { harnessFailure: { message: 'timeout' } })]);
  assert.equal(observed.correctlyHandled, 1); assert.equal(observed.quietCorrectControls, 0);
  assert.equal(observed.pairsPassed, 0); assert.deepEqual(observed.harnessFailures, ['broken']);
});
test('published typing errors suppress every detector channel before scoring', () => {
  const detected = familyFeedback({ publishedTypingErrors: [{ code: 2339 }], feedback: [{ code: 'STRICT_READ_UNTRACKED' }],
    errors: [{ message: 'failure' }] }, { warnings: [{ rule: 'reactive-read-after-await' }],
    baseline: { findings: [{ kind: 'violation', rule: 'reactive-read-after-await' }] }, getter: { notes: [{ code: 'candidate' }] } });
  assert.deepEqual(detected.feedback, []); assert.deepEqual(detected.gaps, []);
  const observed = scoreHoldout([row('typing', 'target', [], detected)]);
  assert.equal(observed.typeExcluded, 1); assert.equal(observed.executed, 0); assert.equal(observed.correctlyHandled, 0);
});
