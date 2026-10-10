import assert from 'node:assert/strict';
import { mkdtempSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';
import { read } from './catalog.mjs';
import { ts } from './lower.mjs';
import { instrumentGuards } from './guard-trace.mjs';
import { getterSnapshotsV6 } from './snapshot-feedback-v6.mjs';
import { oracleCompilerOptions } from '../../scripts/tsc-oracle.mjs';
const project = read('rust/target/family-holdout-detector-freeze.json').packages.find(row => row.package === '@solid-primitives/mouse').project;
function run(initializer, display, imports = '', helpers = '', needle = "position['x']") {
  const root = mkdtempSync(join(tmpdir(), 'snapshot-v6-')), path = join(root, 'App.tsx'), guard = join(root, 'guard.js'),
    guardText = `import { getObserver, createSignal } from 'solid-js'; function read() { if (!getObserver()) return 1; const [value] = createSignal(1); return value(); }`;
  symlinkSync(join(project, 'node_modules'), join(root, 'node_modules'), 'dir'); writeFileSync(guard, guardText);
  writeFileSync(path, `import { createMousePosition } from '@solid-primitives/mouse'; ${imports}
${helpers}
function App() { const position = createMousePosition(window); const frozen = ${initializer}; return <p>{${display}}</p>; }`);
  const options = ts.convertCompilerOptionsFromJson(oracleCompilerOptions('v2', true), root).options,
    program = ts.createProgram([path], options), source = program.getSourceFile(path);
  assert.deepEqual(ts.getPreEmitDiagnostics(program).filter(d => d.category === ts.DiagnosticCategory.Error).map(d => d.code), []);
  const position = source.getLineAndCharacterOfPosition(source.text.indexOf(needle));
  return { program, source, event: { ...instrumentGuards(guardText, guard).observations[0],
    originalLocation: { path, line: position.line + 1, column: position.character + 1 } } };
}
test('retained computed, arithmetic, conditional, container and template reads acquire only observed hints', () => {
  for (const [initializer, display] of [["position['x']", 'frozen'], ["position['x'] + 2", 'frozen'],
    ["position['x'] > 0 ? 7 : 3", 'frozen'], ["[position['x'], position.y]", 'frozen.join(",")'],
    ["({ x: position['x'] })", 'frozen.x'], ["`x:${position['x']}`", 'frozen']]) {
    const { program, source, event } = run(initializer, display), result = getterSnapshotsV6(program, source, [event, event]);
    assert.equal(result.candidates.length, 1, initializer); assert.equal(result.candidates[0].certification, false);
    assert.equal(getterSnapshotsV6(program, source, []).candidates.length, 0);
    assert.equal(getterSnapshotsV6(program, source, [{ ...event, start: 0 }]).candidates.length, 0);
  }
});
test('exact native untrack aliases, namespace members and const aliases suppress observed hints', () => {
  for (const [imports, helper, expression] of [
    ["import { untrack as snapshot } from 'solid-js';", '', "snapshot(() => position['x'])"],
    ["import * as core from 'solid-js';", '', "core.untrack(() => position['x'])"],
    ["import { untrack } from 'solid-js';", 'const snapshot = untrack;', "snapshot(() => position['x'])"],
  ]) {
    const { program, source, event } = run(expression, 'frozen', imports, helper), result = getterSnapshotsV6(program, source, [event]);
    assert.equal(result.candidates.length, 0); assert.equal(result.suppressedObservations[0].reason, 'exact native untrack argument');
  }
});
test('discarded reads, unknown helper arguments, deferred values and invalid locations stay quiet', () => {
  for (const [expression, display, helper] of [["(position['x'], 9)", 'frozen', ''],
    ["ignore(position['x'])", 'frozen', 'function ignore(_value: number) { return 9; }'],
    ["() => position['x']", 'frozen()', ''], ["void position['x']", 'String(frozen)', ''],
    ["position['x']", '(() => frozen)()', '']]) {
    const { program, source, event } = run(expression, display, '', helper);
    assert.equal(getterSnapshotsV6(program, source, [event]).candidates.length, 0, expression);
  }
  const { program, source, event } = run("position['x']", 'frozen');
  for (const location of [{ ...event.originalLocation, line: 0 }, { ...event.originalLocation, line: 999 },
    { ...event.originalLocation, column: 9999 }, { ...event.originalLocation, path: 'other.tsx' }])
    assert.equal(getterSnapshotsV6(program, source, [{ ...event, originalLocation: location }]).candidates.length, 0);
});
