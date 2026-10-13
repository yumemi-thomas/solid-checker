import assert from 'node:assert/strict';
import { mkdtempSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';
import { read, hash } from './catalog.mjs';
import { ts } from './lower.mjs';
import { instrumentGuards } from './guard-trace.mjs';
import { getterSnapshotsV11 } from './snapshot-feedback-v11.mjs';
import { oracleCompilerOptions } from '../../scripts/tsc-oracle.mjs';
const project = read('rust/target/family-holdout-detector-freeze.json').packages.find(row => row.package === '@solid-primitives/mouse').project;
function analyze(helpers, initializer, stackMarkers) {
  const root = mkdtempSync(join(tmpdir(), 'snapshot-v11-')), path = join(root, 'App.tsx'), guard = join(root, 'guard.js'),
    text = `import { getObserver, createSignal } from 'solid-js'; function read() { if (!getObserver()) return 1; const [value] = createSignal(1); return value(); }`;
  symlinkSync(join(project, 'node_modules'), join(root, 'node_modules'), 'dir'); writeFileSync(guard, text);
  writeFileSync(path, `import { createMousePosition } from '@solid-primitives/mouse'; import { untrack } from 'solid-js';
function App() { const position = createMousePosition(window); ${helpers} const frozen = ${initializer}; return <p>{frozen}</p>; }`);
  const program = ts.createProgram([path], ts.convertCompilerOptionsFromJson(oracleCompilerOptions('v2', true), root).options), source = program.getSourceFile(path);
  assert.deepEqual(ts.getPreEmitDiagnostics(program).filter(d => d.category === ts.DiagnosticCategory.Error).map(d => d.code), []);
  const at = marker => {
    const index = marker === 'SETUP' ? source.text.lastIndexOf(initializer) : source.text.indexOf(marker);
    assert(index >= 0); const point = source.getLineAndCharacterOfPosition(index);
    return { path, line: point.line + 1, column: point.character + 1, sourceSha256: hash(source.text) };
  }, originalFrames = stackMarkers.map(at), event = { ...instrumentGuards(text, guard).observations[0], originalFrames, originalLocation: originalFrames[0] };
  return getterSnapshotsV11(program, source, [event]);
}
for (const [label, helpers, initializer, markers] of [
  ['nested transparent callee wrappers', `const read = () => position.x;`, '(read as () => number)()', ['position.x', 'SETUP']],
  ['immutable alias chain', `const read = () => position.x; const first = read; const selected = first;`, 'selected()', ['position.x', 'SETUP']],
  ['property helper parameter', `function read(key: 'x') { return position[key]; }`, "read('x')", ['position[key]', 'SETUP']],
  ['two exact local return edges', `const inner = () => position.x; function read() { return inner(); }`, 'read()', ['position.x', 'inner();', 'SETUP']],
]) test('observed local path admits ' + label, () => assert.equal(analyze(helpers, initializer, markers).candidates.length, 1));
for (const [label, helpers, initializer, markers] of [
  ['mutable alias', `const read = () => position.x; let selected = read;`, 'selected()', ['position.x', 'SETUP']],
  ['member dispatch', `const read = () => position.x; const holder = { read };`, 'holder.read()', ['position.x', 'SETUP']],
  ['discarding intermediate return', `const inner = () => position.x; function read() { return (inner(), 9); }`, 'read()', ['position.x', 'inner(),', 'SETUP']],
  ['unknown call in the returned member', `function ignore(_value: number) { return { x: 9 }; } function read() { return ignore(position.x).x; }`, 'read()', ['position.x', 'SETUP']],
  ['missing intermediate frame', `const inner = () => position.x; function read() { return inner(); }`, 'read()', ['position.x', 'SETUP']],
  ['explicit native untrack return', `const inner = () => position.x; function read() { return untrack(inner); }`, 'read()', ['position.x', 'untrack(inner)', 'SETUP']],
  ['setup argument location', `function read(_key: 'x') { return position.x; }`, "read('x')", ['position.x', "'x')"]],
]) test('observed local path refuses ' + label, () => assert.equal(analyze(helpers, initializer, markers).candidates.length, 0));
