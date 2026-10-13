import assert from 'node:assert/strict';
import { mkdtempSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';
import { read } from './catalog.mjs';
import { ts } from './lower.mjs';
import { instrumentGuards } from './guard-trace.mjs';
import { getterSnapshotsV8 } from './snapshot-feedback-v8.mjs';
import { oracleCompilerOptions } from '../../scripts/tsc-oracle.mjs';
const project = read('rust/target/family-holdout-detector-freeze.json').packages.find(row => row.package === '@solid-primitives/mouse').project;
function run(helper, statement = 'const frozen = readX();', extra = '', display = 'frozen') {
  const root = mkdtempSync(join(tmpdir(), 'snapshot-v8-')), path = join(root, 'App.tsx'), guard = join(root, 'guard.js'),
    guardText = `import { getObserver, createSignal } from 'solid-js'; function read() { if (!getObserver()) return 1; const [value] = createSignal(1); return value(); }`;
  symlinkSync(join(project, 'node_modules'), join(root, 'node_modules'), 'dir'); writeFileSync(guard, guardText);
  writeFileSync(path, `import { createMousePosition } from '@solid-primitives/mouse'; import { untrack } from 'solid-js';
function App() { const position = createMousePosition(window); ${helper} ${extra} ${statement} return <p>{${display}}</p>; }`);
  const options = ts.convertCompilerOptionsFromJson(oracleCompilerOptions('v2', true), root).options,
    program = ts.createProgram([path], options), source = program.getSourceFile(path);
  assert.deepEqual(ts.getPreEmitDiagnostics(program).filter(d => d.category === ts.DiagnosticCategory.Error).map(d => d.code), []);
  const position = source.getLineAndCharacterOfPosition(source.text.indexOf("position['x']"));
  return { program, source, event: { ...instrumentGuards(guardText, guard).observations[0], originalLocation: {
    path, line: position.line + 1, column: position.character + 1 } } };
}
test('exact local function and arrow returns with one setup caller carry observed hints', () => {
  for (const helper of ["function readX() { return position['x']; }", "const readX = () => position['x'];"]) {
    const { program, source, event } = run(helper), result = getterSnapshotsV8(program, source, [event]);
    assert.equal(result.candidates.length, 1); assert.equal(result.candidates[0].localReturn.callerReferences, 1);
    assert.equal(source.text.slice(result.candidates[0].start, result.candidates[0].end), 'readX()');
    assert.equal(getterSnapshotsV8(program, source, []).candidates.length, 0);
    assert.equal(getterSnapshotsV8(program, source, [{ ...event, end: 0 }]).candidates.length, 0);
  }
});
test('multiple callers, escape, writes, async, branches, argument helpers and native snapshots stay open', () => {
  for (const [helper, statement, extra, display] of [
    ["function readX() { return position['x']; }", 'const frozen = readX();', 'readX();', 'frozen'],
    ["function readX() { return position['x']; }", 'const frozen = readX();', 'const escaped = readX;', 'frozen'],
    ["let readX = () => position['x'];", 'const frozen = readX();', 'readX = () => 9;', 'frozen'],
    ["async function readX() { return position['x']; }", 'const frozen = readX();', '', 'String(frozen)'],
    ["function readX() { if (true) return position['x']; return 9; }", 'const frozen = readX();', '', 'frozen'],
    ["function readX(value: number) { return position['x'] + value; }", 'const frozen = readX(1);', '', 'frozen'],
    ["function readX() { return position['x']; }", 'const frozen = untrack(readX);', '', 'frozen'],
  ]) {
    const { program, source, event } = run(helper, statement, extra, display);
    assert.equal(getterSnapshotsV8(program, source, [event]).candidates.length, 0, helper + statement + extra);
  }
});
