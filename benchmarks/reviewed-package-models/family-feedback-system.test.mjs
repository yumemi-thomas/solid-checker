import assert from 'node:assert/strict';
import { test } from 'node:test';
import { familyFeedback } from './family-feedback-system.mjs';
test('feedback cannot be selected by authored target labels or expected rules', () => {
  const observed = { publishedTypingErrors: [], feedback: [{ code: 'REAL_OBSERVATION', certification: false }], errors: [] };
  const source = { warnings: [{ rule: 'actual-source-warning', certification: false }], baseline: { findings: [
    { kind: 'uncertifiable', rule: 'package-contract-incomplete' },
  ] } };
  const actual = familyFeedback(observed, source);
  for (const provenance of [{ role: 'target', expectedIssue: true, rules: ['invented-rule'] }, { role: 'control', expectedIssue: false }])
    assert.deepEqual(familyFeedback({ ...observed, provenance }, source), actual);
  assert.equal(actual.feedback.length, 2); assert.equal(actual.gaps.length, 1);
});
test('published TypeScript errors exclude every checker channel', () => {
  const result = familyFeedback({ publishedTypingErrors: [{ code: 2540 }], feedback: [{ code: 'noise' }], errors: ['noise'] },
    { baseline: { findings: [{ kind: 'violation' }] }, warnings: [{ rule: 'duplicate' }], getter: { notes: [{}] } });
  assert.deepEqual(result, { excluded: 'TypeScript owns this input', feedback: [], gaps: [] });
});
test('plain exceptions stay exceptions without guessing a semantic rule', () => {
  const error = { message: '[FLUSH_IN_ACTION] operation rejected', originalLocation: { path: '/app.ts', line: 2 } };
  const result = familyFeedback({ publishedTypingErrors: [], feedback: [], errors: [error], pageErrors: [error] });
  assert.equal(result.feedback.length, 1); assert.equal(result.feedback[0].channel, 'runtime-exception');
  assert.equal(result.feedback[0].rule, undefined); assert.equal(result.feedback[0].certification, false);
});
