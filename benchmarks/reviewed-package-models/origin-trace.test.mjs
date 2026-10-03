import assert from "node:assert/strict";
import { test } from "node:test";
import { instrumentOrigins, instrumentSettledFire } from "./origin-trace.mjs";
import { collectPackageOrigins } from "./origin-trace-runtime.mjs";
import { runtimeFeedback } from "./extended-runtime-feedback.mjs";
test('origin instrumentation resolves aliases and refuses shadowed registrations', () => {
  const source = `import { onSettled as settle } from 'solid-js'; function run(fn) { settle(fn); }`;
  const traced = instrumentOrigins(source, '/package/index.js'); assert.equal(traced.registrations.length, 1);
  for (const value of [`function onSettled(fn) {} onSettled(() => {});`, `import { onSettled } from 'solid-js'; function run(onSettled) { onSettled(() => {}); }`, `import { onSettled } from 'solid-js'; onSettled(...args);`]) assert.equal(instrumentOrigins(value, '/package/index.js'), null);
});
test('late callback origin preserves this, arguments, cleanup return and thrown identity', () => {
  const collector = collectPackageOrigins(), premise = { path: '/package/index.js', start: 1 }, cleanup = () => {}, receiver = {};
  let input = 0;
  const callback = collector.wrap(function (value) { assert.equal(this, receiver); input += value; assert.equal(collector.current().premise, premise); return cleanup; }, premise);
  assert.equal(callback.call(receiver, 2), cleanup); assert.equal(input, 2); assert.equal(collector.current(), null);
  const thrown = new Error('late failure');
  assert.throws(collector.wrap(() => { throw thrown; }, premise), error => error === thrown);
  assert.equal(collector.failures[0].message, thrown.message); assert.equal(collector.current(), null);
  const source = `import { onSettled } from 'solid-js'; let calls = 0; function make() { calls++; return () => 1; } onSettled(make()); return calls;`;
  const traced = instrumentOrigins(source, '/package/index.js'); const previous = globalThis.__packageOrigins; globalThis.__packageOrigins = collector;
  try { const execute = value => new Function('onSettled', value.replace(/import[^;]+;/, ''))(fn => assert.equal(fn(), 1)); assert.equal(execute(source), execute(traced.code)); }
  finally { globalThis.__packageOrigins = previous; }
});
test('expanded diagnostic policy preserves advisory severity and excludes type diagnostics', () => {
  let listener; const run = runtimeFeedback({ diagnostics: { subscribe(fn) { listener = fn; return () => {}; } } }, { appRoot: '/consumer' });
  listener({ code: 'MISSING_EFFECT_FN', severity: 'error', message: 'arity' });
  listener({ code: 'INVALID_REFRESH_TARGET', severity: 'error', message: 'shape' });
  assert.equal(run.feedback.length, 0);
  listener({ code: 'SETTLED_CLEANUP_UNOWNED', severity: 'error', message: 'cleanup' });
  listener({ code: 'EFFECT_WRITES_OWN_SOURCE', severity: 'info', message: 'relay' });
  assert.equal(run.feedback[0].category, 'execution'); assert.equal(run.feedback[1].category, 'advisory');
  assert.equal(run.feedback[1].severity, 'info'); assert.equal(run.feedback[1].certification, false);
});
test('runtime fire preserves origin after cleanup return and restores it on rejection', () => {
  const collector = collectPackageOrigins(), premise = { path: '/package/index.js', start: 1 };
  const callback = collector.wrap(() => () => {}, premise);
  assert.equal(collector.current(), null);
  const thrown = new Error('dropped cleanup');
  assert.throws(() => collector.invokeSettled(callback, () => { const cleanup = callback(); assert.equal(typeof cleanup, 'function'); assert.equal(collector.current().premise, premise); throw thrown; }), error => error === thrown);
  assert.equal(collector.current(), null); assert.equal(collector.failures.length, 1);
  collector.invokeSettled(() => {}, () => assert.equal(collector.current(), null));
  const source = `function onSettled(callback) { return function fire() { const cleanup = callback(); if (cleanup !== undefined) { const event = { code: "SETTLED_CLEANUP_UNOWNED" }; throw new Error(event.code); } }; }`;
  const traced = instrumentSettledFire(source, '/core/dev.js'); assert.equal(traced.registrations.length, 1);
  const previous = globalThis.__packageOrigins; globalThis.__packageOrigins = collector;
  try {
    const execute = value => new Function(value + '; return onSettled;')();
    assert.equal(execute(source)(() => {})(), execute(traced.code)(() => {})());
    assert.throws(execute(traced.code)(callback), /SETTLED_CLEANUP_UNOWNED/);
  } finally { globalThis.__packageOrigins = previous; }
});
