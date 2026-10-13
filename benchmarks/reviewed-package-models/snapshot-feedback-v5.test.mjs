import assert from 'node:assert/strict';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';
import { ts } from './lower.mjs';
import { instrumentGuards } from './guard-trace.mjs';
import { getterSnapshotsV5 } from './snapshot-feedback-v5.mjs';
function run(statement, display, callback = false) {
  const root = mkdtempSync(join(tmpdir(), 'snapshot-v5-')), path = join(root, 'App.tsx'), guard = join(root, 'guard.js'),
    text = `import { getObserver, createSignal } from 'solid-js'; function read() { if (!getObserver()) return 1; const [value] = createSignal(1); return value(); }`;
  writeFileSync(guard, text);
  writeFileSync(path, `function App() { ${callback ? 'return unknownCallback(() => {' : ''} const ${statement}; return <p>{${display}}</p>; ${callback ? '});' : ''} }`);
  const program = ts.createProgram([path], { noLib: true, jsx: ts.JsxEmit.Preserve }), source = program.getSourceFile(path),
    position = source.getLineAndCharacterOfPosition(source.text.indexOf('const ') + 6),
    event = { ...instrumentGuards(text, guard).observations[0], originalLocation: {
      path, line: position.line + 1, column: position.character + 1 } };
  // A computed-call event is located at the expression; a destructure event at its binding.
  if (!statement.startsWith('{')) event.originalLocation.column += statement.indexOf('=') + 2;
  return { program, source, event };
}
test('observed computed calls and multiple destructures keep static dispatch open', () => {
  for (const [statement, display] of [['frozen = state[member](key)', 'frozen'], ['{ x, y: other } = state', 'x + other']]) {
    const { program, source, event } = run(statement, display), result = getterSnapshotsV5(program, source, [event, event]);
    assert.equal(result.candidates.length, 1);
    assert.equal(result.candidates[0].code, 'OBSERVED_PACKAGE_SNAPSHOT_FLOW');
    assert.equal(result.candidates[0].staticDispatch, 'open'); assert.equal(result.candidates[0].certification, false);
    assert.equal(getterSnapshotsV5(program, source, []).candidates.length, 0);
    assert.equal(getterSnapshotsV5(program, source, [{ ...event, end: event.end + 1 }]).candidates.length, 0);
  }
});
test('callback, rest, default, deferred JSX and unrelated guard locations do not acquire hints', () => {
  for (const [statement, display, callback] of [['{ x, ...rest } = state', 'x', false], ['{ x = 1, y } = state', 'x', false],
    ['{ x, y } = state', 'x', true], ['frozen = state[member](key)', '() => frozen', false]]) {
    const { program, source, event } = run(statement, display, callback);
    assert.equal(getterSnapshotsV5(program, source, [event]).candidates.length, 0, statement);
  }
  const { program, source, event } = run('frozen = state[member](key)', 'frozen');
  assert.equal(getterSnapshotsV5(program, source, [{ ...event, originalLocation: { ...event.originalLocation, column: 1 } }]).candidates.length, 0);
});
