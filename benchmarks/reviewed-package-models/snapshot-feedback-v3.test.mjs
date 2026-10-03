import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';
import { hash } from './catalog.mjs';
import { ts } from './lower.mjs';
import { instrumentGuards } from './guard-trace.mjs';
import { authenticatedGetterSnapshots } from './snapshot-feedback-v3.mjs';
test('arbitrary bytes and fabricated guard spans cannot become snapshot hints', () => {
  const root = mkdtempSync(join(tmpdir(), 'authenticated-guard-')), path = join(root, 'guard.js'), app = join(root, 'App.tsx'),
    text = `import { getObserver, createSignal } from 'solid-js'; function read() { if (!getObserver()) return 1; const [value] = createSignal(1); return value(); }`;
  writeFileSync(path, text); writeFileSync(app, 'function App() { const value = object.count; return <p>{value}</p>; }');
  const program = ts.createProgram([app], { noLib: true, jsx: ts.JsxEmit.Preserve }), source = program.getSourceFile(app),
    position = source.getLineAndCharacterOfPosition(source.text.indexOf('object.count')),
    event = { ...instrumentGuards(text, path).observations[0], originalLocation: {
      path: app, line: position.line + 1, column: position.character + 1 } };
  assert.equal(authenticatedGetterSnapshots(program, source, [event]).candidates.length, 1);
  for (const changed of [{ ...event, start: 0 }, { ...event, end: event.end + 1 }, { ...event, sourceSha256: hash('wrong') }])
    assert.equal(authenticatedGetterSnapshots(program, source, [changed]).candidates.length, 0);
  writeFileSync(path, 'ordinary getter');
  assert.equal(authenticatedGetterSnapshots(program, source, [{ ...event, sourceSha256: hash(readFileSync(path)) }]).candidates.length, 0);
});
