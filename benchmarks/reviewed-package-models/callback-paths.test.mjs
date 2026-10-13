import assert from 'node:assert/strict';
import test from 'node:test';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { CallbackPaths, callbackArguments } from './callback-paths.mjs';
import { instrumentCallbackArguments } from './callback-instrument.mjs';
import { ts } from './lower.mjs';
function fixture(text, argument = '() => 1') {
  const path = join(mkdtempSync(join(tmpdir(), 'solid-callback-path-')), 'index.js'); writeFileSync(path, text);
  const source = ts.createSourceFile('consumer.ts', `candidate(${argument});`, ts.ScriptTarget.Latest, true);
  return new CallbackPaths('browser').profile(path, 'candidate', callbackArguments(source.statements[0].expression.arguments).values);
}
test('callback paths distinguish exact memo aliases, explicit untrack and root setup', () => {
  const prefix = `import { createMemo as memo, untrack, createRoot } from 'solid-js';`;
  const direct = fixture(prefix + `export function candidate(fn) { return memo(fn); }`);
  assert(direct.assumptions.some(a => a.context === 'tracked-compute'));
  const untracked = fixture(prefix + `export function candidate(fn) { return memo(() => untrack(fn)); }`);
  assert(untracked.assumptions.some(a => a.context === 'untracked')); assert(!untracked.assumptions.some(a => a.context === 'tracked-compute'));
  const root = fixture(prefix + `export function candidate(fn) { return memo(() => createRoot(() => fn())); }`);
  assert(root.assumptions.some(a => a.context === 'root-setup')); assert(!root.assumptions.some(a => a.context === 'tracked-compute'));
});
test('unknown branch, stored callback, shadowed native and async/generator execution stay open', () => {
  for (const text of [
    `export function candidate(fn, enabled) { if (enabled) fn(); }`,
    `export function candidate(fn) { return () => fn(); }`,
    `function createMemo(fn) { return fn; } export function candidate(fn) { return createMemo(fn); }`,
    `async function later(fn) { await 1; fn(); } export function candidate(fn) { return later(fn); }`,
    `function* later(fn) { fn(); yield 1; } export function candidate(fn) { return later(fn); }`,
  ]) assert.equal(fixture(text).assumptions.length, 0);
});
test('callback instrumentation preserves nested returned functions and expression values', () => {
  const input = ['() => (() => ({ value: 3 }))'], result = instrumentCallbackArguments(input, 'read'), h = { samples: [], stage: 'test' };
  const [outer] = Function('h', 'read', 'set', 'getOwner', 'getObserver', `return [${result.arguments.join(',')}];`)(h, () => 1, () => assert.fail(), () => null, () => null);
  assert.deepEqual(outer()(), { value: 3 }); assert.deepEqual(h.samples.map(s => s.id), [0,1]); assert.equal(result.count, 2);
});
