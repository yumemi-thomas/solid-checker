import assert from 'node:assert/strict';
import { mkdtempSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';
import { read } from './catalog.mjs';
import { ts } from './lower.mjs';
import { instrumentGuards } from './guard-trace.mjs';
import { getterSnapshotsV8 } from './snapshot-feedback-v8.mjs';
import { getterSnapshotsV9 } from './snapshot-feedback-v9.mjs';
import { oracleCompilerOptions } from '../../scripts/tsc-oracle.mjs';
const project = read('rust/target/family-holdout-detector-freeze.json').packages.find(row => row.package === '@solid-primitives/mouse').project;
test('local helpers cannot acquire a retained-value premise from a discarded argument', () => {
  const root = mkdtempSync(join(tmpdir(), 'snapshot-v9-')), path = join(root, 'App.tsx'), guard = join(root, 'guard.js'),
    text = `import { getObserver, createSignal } from 'solid-js'; function read() { if (!getObserver()) return 1; const [value] = createSignal(1); return value(); }`;
  symlinkSync(join(project, 'node_modules'), join(root, 'node_modules'), 'dir'); writeFileSync(guard, text);
  for (const [expression, desired] of [["position['x']", 1], ["ignore(position['x']).x", 0]]) {
    writeFileSync(path, `import { createMousePosition } from '@solid-primitives/mouse';
function App() { const position = createMousePosition(window); function ignore(_value: number) { return { x: 9 }; }
function readX() { return ${expression}; } const frozen = readX(); return <p>{frozen}</p>; }`);
    const options = ts.convertCompilerOptionsFromJson(oracleCompilerOptions('v2', true), root).options,
      program = ts.createProgram([path], options), source = program.getSourceFile(path);
    assert.deepEqual(ts.getPreEmitDiagnostics(program).filter(d => d.category === ts.DiagnosticCategory.Error).map(d => d.code), []);
    const position = source.getLineAndCharacterOfPosition(source.text.indexOf("position['x']")), event = {
      ...instrumentGuards(text, guard).observations[0], originalLocation: { path, line: position.line + 1, column: position.character + 1 } };
    assert.equal(getterSnapshotsV9(program, source, [event]).candidates.length, desired);
    if (!desired) assert.equal(getterSnapshotsV8(program, source, [event]).candidates.length, 1, 'retained falsifier for the earlier version');
  }
});
