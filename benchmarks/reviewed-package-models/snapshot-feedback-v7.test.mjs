import assert from 'node:assert/strict';
import { mkdtempSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';
import { read } from './catalog.mjs';
import { ts } from './lower.mjs';
import { instrumentGuards } from './guard-trace.mjs';
import { getterSnapshotsV7 } from './snapshot-feedback-v7.mjs';
import { oracleCompilerOptions } from '../../scripts/tsc-oracle.mjs';
const project = read('rust/target/family-holdout-detector-freeze.json').packages.find(row => row.package === '@solid-primitives/map').project;
test('source-map call punctuation supports the observed call but not discarded argument reads', () => {
  const root = mkdtempSync(join(tmpdir(), 'snapshot-v7-')), path = join(root, 'App.tsx'), guard = join(root, 'guard.js'),
    guardText = `import { getObserver, createSignal } from 'solid-js'; function read() { if (!getObserver()) return 1; const [value] = createSignal(1); return value(); }`;
  symlinkSync(join(project, 'node_modules'), join(root, 'node_modules'), 'dir'); writeFileSync(guard, guardText);
  writeFileSync(path, `import { ReactiveWeakMap } from '@solid-primitives/map';
function ignore(_value: number | undefined) { return 9; }
function App() { const key = {}; const state = new ReactiveWeakMap<object, number>([[key, 1]]); const member = () => 'get' as const;
const frozen = state[member()](key); const ignored = ignore(state.get(key)); return <p>{frozen + ':' + ignored}</p>; }`);
  const options = ts.convertCompilerOptionsFromJson(oracleCompilerOptions('v2', true), root).options,
    program = ts.createProgram([path], options), source = program.getSourceFile(path);
  assert.deepEqual(ts.getPreEmitDiagnostics(program).filter(d => d.category === ts.DiagnosticCategory.Error).map(d => d.code), []);
  function event(offset) { const location = source.getLineAndCharacterOfPosition(offset); return {
    ...instrumentGuards(guardText, guard).observations[0], originalLocation: {
      path, line: location.line + 1, column: location.character + 1 } }; }
  const call = source.text.indexOf('](key)') + 1, argument = source.text.indexOf('state.get(key)');
  assert.equal(getterSnapshotsV7(program, source, [event(call)]).candidates.length, 1);
  assert.equal(getterSnapshotsV7(program, source, [event(argument)]).candidates.length, 0);
});
