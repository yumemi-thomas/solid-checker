import assert from 'node:assert/strict';
import { mkdtempSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import test from 'node:test';
import { read } from './catalog.mjs';
import { ts } from './lower.mjs';
import { instrumentGuards } from './guard-trace.mjs';
import { classSnapshotFlowsV4, getterSnapshotsV4 } from './snapshot-feedback-v4.mjs';
import { oracleCompilerOptions } from '../../scripts/tsc-oracle.mjs';
const retained = read(resolve('rust/target/family-holdout-detector-freeze.json')).packages.find(row => row.package === '@solid-primitives/map');
function programFor(text) {
  const root = mkdtempSync(join(tmpdir(), 'snapshot-v4-')), path = join(root, 'App.tsx');
  symlinkSync(join(retained.project, 'node_modules'), join(root, 'node_modules'), 'dir'); writeFileSync(path, text);
  const options = ts.convertCompilerOptionsFromJson(oracleCompilerOptions('v2', true), root).options,
    program = ts.createProgram([path], options), source = program.getSourceFile(path);
  assert.equal(ts.getPreEmitDiagnostics(program).filter(d => d.category === ts.DiagnosticCategory.Error).length, 0);
  return { root, program, source };
}
test('class reads inside tracked or unknown inline callbacks remain quiet', () => {
  for (const caller of ['createMemo', 'untrack', 'unknownCallback']) {
    const { program, source } = programFor(`import { ReactiveMap } from '@solid-primitives/map'; import { createMemo, untrack } from 'solid-js';
function unknownCallback<T>(fn: () => T) { return fn(); }
function App() { const state = new ReactiveMap<string, number>([['n', 1]]);
return ${caller}(() => { const frozen = state['get']('n'); return <p>{frozen}</p>; }); }`);
    assert.equal(classSnapshotFlowsV4(program, source).candidates.length, 0, caller);
  }
});
test('single-field destructures need an authentic guard; multiple, rest and callback forms stay open', () => {
  const guardText = `import { getObserver, createSignal } from 'solid-js'; function read() { if (!getObserver()) return 1; const [value] = createSignal(1); return value(); }`;
  for (const [binding, callback, wanted] of [['{ x: frozen }', false, 1], ['{ x: frozen, y }', false, 0],
    ['{ x: frozen, ...rest }', false, 0], ['{ x: frozen }', true, 0]]) {
    const text = `import { createMemo } from 'solid-js'; function App() { const object = { x: 1, y: 2 };
${callback ? 'return createMemo(() => {' : ''} const ${binding} = object; return <p>{frozen}</p>; ${callback ? '});' : ''} }`;
    const { root, program, source } = programFor(text), path = join(root, 'guard.js'); writeFileSync(path, guardText);
    const position = source.getLineAndCharacterOfPosition(source.text.indexOf('= object') + 2),
      event = { ...instrumentGuards(guardText, path).observations[0], originalLocation: {
        path: source.fileName, line: position.line + 1, column: position.character + 1 } };
    assert.equal(getterSnapshotsV4(program, source, [event]).candidates.length, wanted, binding);
    assert.equal(getterSnapshotsV4(program, source, [{ ...event, start: 0 }]).candidates.length, 0);
  }
});
