import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import test from 'node:test';
import { ts } from './lower.mjs';
import { hash, read } from './catalog.mjs';
import { classSnapshotFlowsV2, observedGetterSnapshots, snapshotFeedbackV2 } from './snapshot-feedback-v2.mjs';
import { oracleCompilerOptions } from '../../scripts/tsc-oracle.mjs';
const retained = read(resolve('rust/target/family-holdout-detector-freeze.json')).packages.find(row => row.package === '@solid-primitives/map');
function consumer(setup, expression, key = '') {
  const root = mkdtempSync(join(tmpdir(), 'snapshot-v2-')), path = join(root, 'App.tsx');
  symlinkSync(join(retained.project, 'node_modules'), join(root, 'node_modules'), 'dir');
  writeFileSync(path, `import { ReactiveMap } from '@solid-primitives/map'; import { untrack } from 'solid-js';
export function App() { const state = new ReactiveMap<string, number>([['n', 1]]); ${setup} ${key}
const frozen = ${expression}; return <p>{String(frozen)}</p>; }`);
  const options = ts.convertCompilerOptionsFromJson(oracleCompilerOptions('v2', true, { customConditions: ['browser', 'development'] }), root).options;
  const program = ts.createProgram([path], options), source = program.getSourceFile(path);
  assert.equal(ts.getPreEmitDiagnostics(program).filter(d => d.category === ts.DiagnosticCategory.Error).length, 0);
  return { program, source };
}
test('direct, literal-computed and stable const keys retain the exact published class member', () => {
  for (const [expression, key] of [["state.get('n')", ''], ["state['get']('n')", ''], ["state[key]('n')", "const key = 'get';"]]) {
    const { program, source } = consumer('', expression, key), result = classSnapshotFlowsV2(program, source);
    assert.equal(result.candidates.length, 1, expression); assert.equal(result.candidates[0].severity, 'info');
    assert.equal(result.candidates[0].certification, false); assert.equal(result.originalSourceSha256, hash(source.text));
    assert.equal(source.text.slice(result.candidates[0].start, result.candidates[0].end), expression);
  }
});
test('escaping instances, replaced members and inspected prototypes remain open', () => {
  for (const setup of [
    `const alias = state; console.log(alias);`, `state.get = () => 1;`,
    `ReactiveMap.prototype.get = () => 1;`, `Object.defineProperty(state, 'get', { value: () => 1 });`,
  ]) {
    const { program, source } = consumer(setup, "state['get']('n')"), result = classSnapshotFlowsV2(program, source);
    assert.equal(result.candidates.length, 0, setup);
  }
});
test('explicit untrack and dynamic key expressions do not acquire a class hint', () => {
  for (const [expression, key] of [
    ["untrack(() => state['get']('n'))", ''],
    ["state[key]('n')", "let key: 'get' = 'get';"],
    ["state[key()]('n')", "const key = () => 'get' as const;"],
  ]) {
    const { program, source } = consumer('', expression, key);
    assert.equal(classSnapshotFlowsV2(program, source).candidates.length, 0, expression);
  }
});
test('guard events need exact current bytes, original snapshot location and a direct JSX value use', () => {
  const { program, source } = consumer('', 'state.size'), path = join(dirnameForTest(source.fileName), 'guard.js');
  writeFileSync(path, 'guard specimen'); const start = source.text.indexOf('state.size'), position = source.getLineAndCharacterOfPosition(start);
  const event = { kind: 'tracking-skipped', path, sourceSha256: hash(readFileSync(path)), start: 0, end: 5,
    originalLocation: { path: source.fileName, line: position.line + 1, column: position.character + 1 } };
  assert.equal(observedGetterSnapshots(program, source, [event]).candidates.length, 1);
  for (const changed of [{ ...event, sourceSha256: 'sha256:stale' },
    { ...event, originalLocation: { ...event.originalLocation, column: 1 } }, { ...event, kind: 'automatic-cleanup-skipped' }])
    assert.equal(observedGetterSnapshots(program, source, [changed]).candidates.length, 0);
});
function dirnameForTest(path) { return path.slice(0, path.lastIndexOf('/')); }
test('typing errors suppress the new informational channels too', () => {
  const result = snapshotFeedbackV2({ publishedTypingErrors: [{ code: 2339 }] },
    { classesV2: { candidates: [{ start: 0 }] }, observedGetters: { candidates: [{ start: 0 }] } });
  assert.deepEqual(result.feedback, []); assert.deepEqual(result.gaps, []);
});
