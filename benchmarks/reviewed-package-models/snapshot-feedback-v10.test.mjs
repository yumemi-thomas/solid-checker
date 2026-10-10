import assert from 'node:assert/strict';
import { mkdtempSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';
import { read, hash } from './catalog.mjs';
import { ts } from './lower.mjs';
import { instrumentGuards } from './guard-trace.mjs';
import { getterSnapshotsV10, snapshotFeedbackV10 } from './snapshot-feedback-v10.mjs';
import { oracleCompilerOptions } from '../../scripts/tsc-oracle.mjs';
const project = read('rust/target/family-holdout-detector-freeze.json').packages.find(row => row.package === '@solid-primitives/mouse').project;
function fixture({ extra = 'readX(); h.read = readX;', returned = "position['x']", initializer = 'readX()', frameCaller, badHash = false, missing = false, wrongOrder = false, expectedTyping = [] } = {}) {
  const root = mkdtempSync(join(tmpdir(), 'snapshot-v10-')), path = join(root, 'App.tsx'), guard = join(root, 'guard.js'),
    text = `import { getObserver, createSignal } from 'solid-js'; function read() { if (!getObserver()) return 1; const [value] = createSignal(1); return value(); }`;
  symlinkSync(join(project, 'node_modules'), join(root, 'node_modules'), 'dir'); writeFileSync(guard, text);
  writeFileSync(path, `import { createMousePosition } from '@solid-primitives/mouse'; import { untrack } from 'solid-js';
const h = (globalThis as any).__experiment;
function App() { const position = createMousePosition(window); function ignore(_value: number) { return { x: 9 }; }
function readX() { return ${returned}; } ${extra} const frozen = ${initializer}; return <p>{frozen}</p>; }`);
  const program = ts.createProgram([path], ts.convertCompilerOptionsFromJson(oracleCompilerOptions('v2', true), root).options), source = program.getSourceFile(path);
  const errors = ts.getPreEmitDiagnostics(program).filter(d => d.category === ts.DiagnosticCategory.Error);
  assert.deepEqual(errors.map(d => d.code), expectedTyping);
  if (errors.length) return snapshotFeedbackV10({ publishedTypingErrors: errors });
  const at = index => { const point = source.getLineAndCharacterOfPosition(index); return { path, line: point.line + 1, column: point.character + 1,
    sourceSha256: badHash ? 'sha256:wrong' : hash(source.text) }; },
    returnedFrame = at(source.text.indexOf("position['x']")), callerFrame = at(frameCaller === 'discarded' ? source.text.indexOf('readX();')
      : source.text.lastIndexOf(initializer)), frames = [null, returnedFrame, callerFrame];
  const event = { ...instrumentGuards(text, guard).observations[0], originalLocation: returnedFrame,
    originalFrames: missing ? [returnedFrame] : wrongOrder ? [callerFrame, returnedFrame] : frames };
  return getterSnapshotsV10(program, source, [event]);
}
test('mapped actual caller admits multiple references and an escaped exact direct call', () => {
  const result = fixture(); assert.equal(result.candidates.length, 1);
  const hint = result.candidates[0]; assert.equal(hint.localReturn.callerReferences, 3);
  assert.equal(hint.observedCall.returnFrameIndex, 1); assert.equal(hint.observedCall.callFrameIndex, 2);
  assert.equal(hint.certification, false); assert.equal(hint.severity, 'info');
});
for (const [label, options] of [
  ['different actual caller', { frameCaller: 'discarded' }],
  ['explicit native snapshot', { initializer: 'untrack(readX)' }],
  ['absent caller frame', { missing: true }],
  ['stale consumer bytes', { badHash: true }],
  ['reversed stack', { wrongOrder: true }],
  ['discarded return argument', { returned: "ignore(position['x']).x" }],
]) test('observed helper refuses ' + label, () => assert.equal(fixture(options).candidates.length, 0));
for (const extra of ['readX = () => 9;', '[readX] = [() => 9];']) test('real TypeScript owns function reassignment: ' + extra, () => {
  const result = fixture({ extra, expectedTyping: [2630] });
  assert.equal(result.excluded, 'TypeScript owns this input'); assert.deepEqual(result.feedback, []);
});
