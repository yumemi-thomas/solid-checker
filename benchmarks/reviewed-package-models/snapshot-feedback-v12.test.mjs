import assert from 'node:assert/strict';
import { mkdtempSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';
import { read, hash } from './catalog.mjs';
import { ts } from './lower.mjs';
import { instrumentGuards } from './guard-trace.mjs';
import { LocalCallTargets } from './local-call-targets-v1.mjs';
import { getterSnapshotsV11 } from './snapshot-feedback-v11.mjs';
import { getterSnapshotsV12 } from './snapshot-feedback-v12.mjs';
import { oracleCompilerOptions } from '../../scripts/tsc-oracle.mjs';
const project = read('rust/target/family-holdout-detector-freeze.json').packages.find(row => row.package === '@solid-primitives/mouse').project;
function fixture(helpers, initializer, markers, { badHash = false, noFrames = false, allowLegacyReplay = false } = {}) {
  const root = mkdtempSync(join(tmpdir(), 'snapshot-v12-')), path = join(root, 'App.tsx'), guard = join(root, 'guard.js'),
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
  return { result: getterSnapshotsV12(program, source, [event], { allowLegacyReplay }), program, source, event };
}
for (const [label, helpers, initializer] of [
  ['own shorthand', `const read = () => position.x; const holder = { read };`, 'holder.read()'],
  ['assigned field', `const read = () => position.x; const holder = { value: read };`, 'holder.value()'],
  ['inline method', `const holder = { read() { return position.x; } };`, 'holder.read()'],
  ['inline arrow', `const holder = { read: () => position.x };`, 'holder.read()'],
  ['literal computed member', `const read = () => position.x; const holder = { read };`, "holder['read']()"],
  ['constant computed member', `const read = () => position.x; const holder = { read }; const key = 'read';`, 'holder[key]()'],
  ['immutable receiver alias', `const read = () => position.x; const holder = { read }; const selected = holder;`, 'selected.read()'],
  ['extracted function alias', `const read = () => position.x; const holder = { read }; const selected = holder.read;`, 'selected()'],
]) test('stable member admits ' + label, () => {
  const { result } = fixture(helpers, initializer, ['position.x', 'SETUP']);
  assert.equal(result.candidates.length, 1); assert.equal(result.candidates[0].observedPath.returns[0].members.length, 1);
  assert.equal(result.candidates[0].certification, false);
});
for (const [label, helpers, initializer] of [
  ['member replacement', `const read = () => position.x; const holder = { read }; holder.read = () => 9;`, 'holder.read()'],
  ['replacement through receiver alias', `const read = () => position.x; const holder = { read }; const selected = holder; selected.read = () => 9;`, 'holder.read()'],
  ['receiver escape', `const read = () => position.x; const holder = { read }; consume(holder);`, 'holder.read()'],
  ['spread receiver', `const read = () => position.x; const holder = { ...{read} };`, 'holder.read()'],
  ['accessor field', `const read = () => position.x; const holder = { get read() { return read; } };`, 'holder.read()'],
  ['receiver dependent method', `const holder = { value: position, read() { return this.value.x; } };`, 'holder.read()'],
  ['dynamic key', `const read = () => position.x; const holder = { read }; let key: 'read' = 'read';`, 'holder[key]()'],
  ['explicit snapshot', `const read = () => position.x; const holder = { read };`, 'untrack(() => holder.read())'],
]) test('stable member refuses ' + label, () => assert.equal(fixture(helpers, initializer, [helpers.includes('this.value.x') ? 'this.value.x' : 'position.x', 'SETUP']).result.candidates.length, 0));
test('finite helper path admits twelve helpers with matching actual frames', () => {
  const helpers = 'function step1() { return position.x; } ' + Array.from({ length: 11 }, (_, index) => `function step${index + 2}() { return step${index + 1}(); }`).join(' '),
    markers = ['position.x', ...Array.from({ length: 11 }, (_, index) => `step${index + 1}();`), 'SETUP'],
    { result } = fixture(helpers, 'step12()', markers);
  assert.equal(result.candidates.length, 1); assert.equal(result.candidates[0].observedPath.returns.length, 12);
});
test('a cyclic local call graph terminates without a candidate', () => {
  const { program, source } = fixture(`function first(): number { return second(); } function second(): number { return first(); }`, 'first()', ['SETUP']);
  const targets = new LocalCallTargets(program, source), call = targets.calls.find(node => node.getText(source) === 'first()');
  assert.equal(targets.chain(targets.target(call.expression)), null);
});
test('all observed channels reject stale consumer bytes', () => {
  const { result, program, source, event } = fixture('', 'position.x', ['SETUP'], { badHash: true });
  assert.equal(result.candidates.length, 0);
  assert.equal(getterSnapshotsV11(program, source, [event]).candidates.length, 1, 'retained earlier stale-observation falsifier');
});
test('nearest-frame legacy observations need explicit authenticated replay mode', () => {
  assert.equal(fixture('', 'position.x', ['SETUP'], { noFrames: true }).result.candidates.length, 0);
  assert.equal(fixture('', 'position.x', ['SETUP'], { noFrames: true, allowLegacyReplay: true }).result.candidates.length, 1);
});
