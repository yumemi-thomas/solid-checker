import assert from 'node:assert/strict';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';
import { GetterPaths } from './getter-paths.mjs';
import { literal, objectValue, unknownValue } from './source-extractor.mjs';
function specimen(text, args = []) {
  const root = mkdtempSync(join(tmpdir(), 'getter-path-')), path = join(root, 'index.js'); writeFileSync(path, text);
  return new GetterPaths().extractGetters(path, 'example', args);
}
test('a returned literal getter retains its exact signal producer', () => {
  const result = specimen(`import { createSignal } from 'solid-js'; export function example() {
    const [read] = createSignal(0); return { get count() { return read(); } }; }`);
  assert.deepEqual(result.fields.map(field => field.path), [['count']]);
  assert.equal(result.fields[0].reads[0].producer.native, 'solid-js.createSignal');
});
test('namespace and aliased core producers retain their semantic identity', () => {
  for (const producer of [
    [`import { createSignal as signal } from 'solid-js';`, 'signal'],
    [`import * as Core from 'solid-js';`, 'Core.createSignal'],
  ]) {
    const result = specimen(`${producer[0]} export function example() { const [read] = ${producer[1]}(0); return { get count() { return read(); } }; }`);
    assert.equal(result.fields.length, 1); assert.equal(result.fields[0].reads[0].producer.native, 'solid-js.createSignal');
  }
});
test('finite getter installation keeps each concrete field and captured loop key', () => {
  const result = specimen(`import { createMemo } from 'solid-js'; export function example(init) {
    const store = { ...init }; for (const key in init) { const value = createMemo(() => init[key]); Object.defineProperty(store, key, { get: () => value() }); } return [store]; }`,
    [objectValue({ count: literal(0), title: literal('one') })]);
  assert.deepEqual(result.fields.map(field => field.path), [[0, 'count'], [0, 'title']]);
});
test('unknown keys and shadowed Object do not manufacture getter behavior', () => {
  const body = `const store = { ...init }; for (const key in init) Object.defineProperty(store, key, { get: () => read() }); return store;`;
  const prefix = `import { createMemo } from 'solid-js'; export function example(init) { const read = createMemo(() => 0);`;
  assert.equal(specimen(prefix + body + '}', [unknownValue('parameter')]).fields.length, 0);
  assert.equal(specimen(prefix + `const Object = { defineProperty() {} };` + body + '}', [objectValue({ count: literal(0) })]).fields.length, 0);
});
test('ordinary getters and unknown computations carry no reactive producer premise', () => {
  for (const text of [
    `export function example() { return { get count() { return 42; } }; }`,
    `export function example() { const reader = foreign(); return { get count() { return reader(); } }; }`,
    `import { createMemo } from 'solid-js'; export function example() { const createMemo = () => () => 42; const read = createMemo(); return { get count() { return read(); } }; }`,
  ]) assert.equal(specimen(text).fields.length, 0);
});
test('finite Object.keys iteration installs getters without wildcard properties', () => {
  const result = specimen(`import { createMemo } from 'solid-js'; export function example() {
    const keys = { width: 0, height: 0 }, out = {}, value = createMemo(() => keys);
    Object.keys(keys).forEach(key => Object.defineProperty(out, key, { get: () => value()[key] })); return out; }`);
  assert.deepEqual(result.fields.map(field => field.path), [['width'], ['height']]);
});
test('every returned alternative needs the same field and a positive producer path', () => {
  const prefix = `import { createSignal } from 'solid-js'; export function example(flag) { const [read] = createSignal(0);`;
  const supported = specimen(prefix + `if (flag) return { get count() { return read(); } }; return { get count() { return read(); } }; }`, [unknownValue('flag')]);
  assert.deepEqual(supported.fields.map(field => field.path), [['count']]);
  for (const other of [`return { get count() { return 42; } };`, `return { get other() { return read(); } };`])
    assert.equal(specimen(prefix + `if (flag) return { get count() { return read(); } }; ${other} }`, [unknownValue('flag')]).fields.length, 0);
});
test('prototype initializers and unresolved loops remain open', () => {
  assert.equal(specimen(`import { createMemo } from 'solid-js'; export function example() { const read = createMemo(() => 0); return { __proto__: {}, get count() { return read(); } }; }`).fields.length, 0);
  assert.equal(specimen(`import { createMemo } from 'solid-js'; export function example(init) { const store = {}, read = createMemo(() => 0); for (const key in init) Object.defineProperty(store, key, { get: () => read() }); return store; }`, [unknownValue('keys')]).fields.length, 0);
});
