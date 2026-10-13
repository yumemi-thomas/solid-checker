import assert from 'node:assert/strict';
import { test } from 'node:test';
import { existsSync, mkdtempSync, symlinkSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { read } from './catalog.mjs';
import { CallbackPaths } from './callback-paths.mjs';
import { unknownValue } from './source-extractor.mjs';
import { installedCatalog } from './demand-models.mjs';
import { phaseSpecializer, trackedCallbackPremise } from './family-phase-specializer.mjs';
import { callbackSourceMap, phaseWarnings } from './family-phase-projection.mjs';
import { familyRequests } from './family-imports.mjs';
import { lower, ts } from './lower.mjs';
import { oracleCompilerOptions } from '../../scripts/tsc-oracle.mjs';
function profile(text, host = 'browser') {
  const path = join(mkdtempSync(join(tmpdir(), 'solid-phase-model-')), 'index.js'); writeFileSync(path, text);
  return new CallbackPaths(host).profile(path, 'candidate', [{ kind: 'callback-probe', id: 0, returnValue: unknownValue('opaque return') }]);
}
test('native tracked forwarding admits the callback phase through an exact alias', () => {
  const observed = profile(`import { createMemo as native } from 'solid-js'; export function candidate(calc) {
    let value; return native(() => { value = calc(value); return value; }); }`);
  assert(trackedCallbackPremise(observed, 0));
  assert.equal(trackedCallbackPremise(observed, 1), false);
});
test('untracked, delayed, conditional, mixed and shadowed callbacks cannot acquire a tracked premise', () => {
  const prefix = `import { createMemo as memo, untrack } from 'solid-js';`;
  const counterexamples = [
    prefix + `export function candidate(fn) { return memo(() => untrack(fn)); }`,
    prefix + `export function candidate(fn) { return memo(() => () => fn()); }`,
    prefix + `export function candidate(fn, enabled) { return memo(() => { if (enabled) fn(); }); }`,
    prefix + `export function candidate(fn) { untrack(fn); return memo(fn); }`,
    prefix + `async function later(fn) { await 1; return fn(); } export function candidate(fn) { return memo(() => later(fn)); }`,
    `function memo(fn) { return fn(); } export function candidate(fn) { return memo(fn); }`,
  ];
  for (const text of counterexamples) assert.equal(trackedCallbackPremise(profile(text), 0), false, text);
});
function consumer(code, host = 'browser') {
  const retained = read(resolve('rust/target/primitives-checkpoint/run-browser.json'));
  const installed = retained.results.find(row => row.package === '@solid-primitives/memo')?.retainedArtifacts.projectDir;
  assert(installed && existsSync(installed), 'Real published package install is required');
  const root = mkdtempSync(join(tmpdir(), 'solid-phase-consumer-')); symlinkSync(join(installed, 'node_modules'), join(root, 'node_modules'), 'dir');
  const path = join(root, 'App.tsx'); writeFileSync(path, code);
  const options = ts.convertCompilerOptionsFromJson(oracleCompilerOptions('v2', true, { customConditions: [host, 'development'] }), root).options;
  const program = ts.createProgram([path], options), source = program.getSourceFile(path);
  assert.equal(ts.getPreEmitDiagnostics(program).filter(d => d.category === ts.DiagnosticCategory.Error).length, 0);
  const imports = familyRequests(program, source, ['@solid-primitives/memo']), catalog = installedCatalog(root, imports.requests, host);
  const lowered = lower(program, source, catalog, host, undefined, phaseSpecializer(catalog, root));
  const modeledPath = join(root, 'model.tsx'); writeFileSync(modeledPath, lowered.text);
  const modeledProgram = ts.createProgram([modeledPath], options);
  assert.equal(ts.getPreEmitDiagnostics(modeledProgram).filter(d => d.category === ts.DiagnosticCategory.Error).length, 0);
  return { program, source, lowered, modeledPath, modeledProgram };
}
const prefix = `import { createSignal } from 'solid-js'; import { createLazyMemo } from '@solid-primitives/memo';
const [read] = createSignal(1);`;
test('installed package phase is host-specific and retains source references', () => {
  const browser = consumer(prefix + `const result = createLazyMemo(async () => { await Promise.resolve(); return read(); });`);
  assert.equal(browser.lowered.sites[0].behavior.trackedCallback, 0);
  assert(browser.lowered.sites[0].extraction.callbackPhase.sourcePins.length);
  const server = consumer(prefix + `const result = createLazyMemo(async () => { await Promise.resolve(); return read(); });`, 'node');
  assert.equal(server.lowered.sites[0].behavior.trackedCallback, undefined);
});
test('Unicode and identical callback text retain distinct exact source provenance', () => {
  const input = consumer(`// 日本語\n${prefix}
const a = createLazyMemo(async () => { await Promise.resolve(); return read(); });
const b = createLazyMemo(async () => { await Promise.resolve(); return read(); });`);
  const mapped = callbackSourceMap(input.lowered, input.source, input.modeledProgram, input.modeledPath);
  assert.equal(mapped.evidence.length, 2); assert.equal(mapped.gaps.length, 0);
  assert.notEqual(mapped.evidence[0].originalStart, mapped.evidence[1].originalStart);
  const modeled = input.modeledProgram.getSourceFile(input.modeledPath), callbacks = mapped.evidence;
  for (const mapping of callbacks) {
    const callOffset = input.lowered.text.indexOf('read()', mapping.start);
    assert(callOffset < mapping.end);
    const finding = { kind: 'violation', rule: 'reactive-read-after-await', message: 'native read after await', primaryLocation: {
      path: input.modeledPath, startByte: Buffer.byteLength(modeled.text.slice(0, callOffset)), endByte: Buffer.byteLength(modeled.text.slice(0, callOffset + 4)) } };
    const projected = phaseWarnings([finding], input.lowered, input.source, input.program, input.modeledProgram, input.modeledPath);
    assert.equal(projected.warnings.length, 1);
    const originalOffset = mapping.originalStart + callOffset - mapping.start;
    assert.equal(projected.warnings[0].location.startByte, Buffer.byteLength(input.source.text.slice(0, originalOffset)));
    assert.equal(projected.warnings[0].certification, false);
  }
});
test('text matching cannot supply provenance without the generated operation premise', () => {
  const input = consumer(prefix + `const result = createLazyMemo(async () => { await Promise.resolve(); return read(); });`);
  const changed = { ...input.lowered, segments: input.lowered.segments.map(segment => segment.copied ? segment : { ...segment, originalStart: -1 }) };
  const mapped = callbackSourceMap(changed, input.source, input.modeledProgram, input.modeledPath);
  assert.equal(mapped.evidence.length, 0); assert.equal(mapped.gaps.length, 1);
});
