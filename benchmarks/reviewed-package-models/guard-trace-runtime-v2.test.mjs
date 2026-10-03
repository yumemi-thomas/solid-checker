import assert from 'node:assert/strict';
import test from 'node:test';
import { collectGuardTrace } from './guard-trace-runtime-v2.mjs';
import { collectGuardTrace as legacyCollector } from './guard-trace-runtime.mjs';
test('same helper retains separate actual caller stacks while repeated stacks coalesce', () => {
  for (const [collect, expected] of [[collectGuardTrace, 2], [legacyCollector, 1]]) {
    const trace = collect(frame => frame.path.endsWith('/guard-trace-runtime-v2.test.mjs'));
    function read() { trace.record({ path: '/package.js', start: 1, kind: 'tracking-skipped' }); }
    function discarded() { read(); }
    function displayed() { read(); }
    for (let index = 0; index < 3; index++) discarded();
    for (let index = 0; index < 3; index++) displayed();
    assert.equal(trace.events.length, expected);
    assert.equal(trace.events[0].certification, false);
  }
});
