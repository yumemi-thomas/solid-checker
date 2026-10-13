import assert from "node:assert/strict";
import { test } from "node:test";
import { instrumentGuards } from "./guard-trace.mjs";
test('guard tracing preserves the actual branch value and cleanup timing', () => {
  const source = `import { getOwner as owner, onCleanup as cleanup } from 'solid-js'; export function run(value) { if (owner()) cleanup(value); return value; }`;
  const result = instrumentGuards(source, '/package/index.js'); assert.equal(result.observations.length, 1);
  const execute = text => new Function('owner', 'cleanup', text.replace(/import[^;]+;/, '').replace('export ', '') + '; return run;');
  const previous = globalThis.__solidGuardTrace, notes = [], cleanups = [];
  globalThis.__solidGuardTrace = { record: value => notes.push(value) };
  try {
    for (const owner of [null, false, {}, undefined]) {
      const original = execute(source)(() => owner, value => cleanups.push(value)); const traced = execute(result.code)(() => owner, value => cleanups.push(value));
      assert.equal(original('value'), traced('value'));
    }
    assert.equal(cleanups.length, 2); assert.equal(notes.length, 3);
  } finally { globalThis.__solidGuardTrace = previous; }
});
test('guard tracing refuses shadowed core names and arbitrary observer guards', () => {
  for (const source of [
    `import { getOwner, onCleanup } from 'solid-js'; function run(getOwner) { if (getOwner()) onCleanup(() => {}); }`,
    `function getOwner() {} function onCleanup() {} if (getOwner()) onCleanup(() => {});`,
    `import { getObserver } from 'solid-js'; function read() { if (!getObserver()) return 1; return 2; }`,
    `import { getObserver, createSignal } from 'solid-js'; function read() { if (!getObserver()) return 1; const deferred = () => createSignal(1); return 2; }`,
  ]) assert.equal(instrumentGuards(source, '/package/index.js'), null);
});
test('tracking bypass requires core source creation in the same function', () => {
  const source = `import { getObserver as observer, createSignal } from 'solid-js'; function read(key) { if (!observer()) return values[key]; const [get] = createSignal(values[key]); return get(); }`;
  const result = instrumentGuards(source, '/package/index.js'); assert.equal(result.observations.length, 1); assert.equal(result.observations[0].kind, 'tracking-skipped');
});
