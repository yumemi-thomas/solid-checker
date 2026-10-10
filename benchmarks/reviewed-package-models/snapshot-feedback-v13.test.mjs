import assert from 'node:assert/strict';
import { mkdtempSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';
import { read, hash } from './catalog.mjs';
import { ts } from './lower.mjs';
import { instrumentGuards } from './guard-trace.mjs';
import { LocalCallTargets } from './local-call-targets-v2.mjs';
import { getterSnapshotsV11 } from './snapshot-feedback-v11.mjs';
import { getterSnapshotsV13 } from './snapshot-feedback-v13.mjs';
import { oracleCompilerOptions } from '../../scripts/tsc-oracle.mjs';
const project = read('rust/target/family-holdout-detector-freeze.json').packages.find(row => row.package === '@solid-primitives/mouse').project;
function fixture(helpers, initializer, markers, { badHash = false, noFrames = false, allowLegacyReplay = false } = {}) {
  const root = mkdtempSync(join(tmpdir(), 'snapshot-v13-')), path = join(root, 'App.tsx'), guard = join(root, 'guard.js'),
    text = `import { getObserver, createSignal } from 'solid-js'; function read() { if (!getObserver()) return 1; const [value] = createSignal(1); return value(); }`;
  symlinkSync(join(project, 'node_modules'), join(root, 'node_modules'), 'dir'); writeFileSync(guard, text);
  writeFileSync(path, `import { createMousePosition } from '@solid-primitives/mouse'; import { untrack } from 'solid-js';
const h = (globalThis as any).__experiment;
function App() { const position = createMousePosition(window); function consume(_value: unknown) {} ${helpers}
const frozen = ${initializer}; return <p>{frozen}</p>; }`);
  const program = ts.createProgram([path], ts.convertCompilerOptionsFromJson(oracleCompilerOptions('v2', true), root).options), source = program.getSourceFile(path);
  assert.deepEqual(ts.getPreEmitDiagnostics(program).filter(d => d.category === ts.DiagnosticCategory.Error).map(d => d.code), []);
  const at = marker => {
    const index = marker === 'SETUP' ? source.text.lastIndexOf(initializer) : source.text.indexOf(marker);
    assert(index >= 0); const point = source.getLineAndCharacterOfPosition(index);
    return { path, line: point.line + 1, column: point.character + 1, sourceSha256: badHash ? 'sha256:stale' : hash(source.text) };
  }, frames = markers.map(at), event = { ...instrumentGuards(text, guard).observations[0], originalLocation: frames[0],
    ...(noFrames ? {} : { originalFrames: frames }) };
  return { result: getterSnapshotsV13(program, source, [event], { allowLegacyReplay }), program, source, event };
}
for (const [label, helpers, initializer] of [
  ['two nested literal members', `const read = () => position.x; const holder = { api: { read } };`, 'holder.api.read()'],
  ['nested receiver alias', `const read = () => position.x; const holder = { api: { read } }; const selected = holder.api;`, 'selected.read()'],
  ['three nested members and an inline method', `const holder = { api: { inner: { read() { return position.x; } } } };`, 'holder.api.inner.read()'],
]) test('nested member admits ' + label, () => assert.equal(fixture(helpers, initializer, ['position.x', 'SETUP']).result.candidates.length, 1));
for (const [label, helpers, initializer] of [
  ['nested method replacement', `const read = () => position.x; const holder = { api: { read } }; holder.api.read = () => 9;`, 'holder.api.read()'],
  ['nested object replacement', `const read = () => position.x; const holder = { api: { read } }; holder.api = { read: () => 9 };`, 'holder.api.read()'],
  ['replacement through nested alias', `const read = () => position.x; const holder = { api: { read } }; const selected = holder.api; selected.read = () => 9;`, 'holder.api.read()'],
  ['nested object escape', `const read = () => position.x; const holder = { api: { read } }; consume(holder.api);`, 'holder.api.read()'],
  ['root object escape', `const read = () => position.x; const holder = { api: { read } }; consume(holder);`, 'holder.api.read()'],
  ['nested accessor', `const read = () => position.x; const holder = { get api() { return {read}; } };`, 'holder.api.read()'],
]) test('nested member refuses ' + label, () => assert.equal(fixture(helpers, initializer, ['position.x', 'SETUP']).result.candidates.length, 0));
