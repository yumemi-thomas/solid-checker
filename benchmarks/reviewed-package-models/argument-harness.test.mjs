import assert from 'node:assert/strict';
import test from 'node:test';
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { spawnSync } from 'node:child_process';
test('an empty input selection cannot execute the browser harness default cases', () => {
  const dir = mkdtempSync(join(tmpdir(), 'solid-empty-witness-')), selection = join(dir, 'selection.json');
  writeFileSync(selection, JSON.stringify({ rows: [], catalogPath: '/missing/catalog' }));
  const result = spawnSync(process.execPath, [new URL('./observation-browser-run.mjs', import.meta.url).pathname,
    new URL('./argument-cases.mjs', import.meta.url).pathname, new URL('./extended-runtime-feedback.mjs', import.meta.url).pathname,
    join(dir, 'study'), '/missing/browser', selection], { encoding: 'utf8' });
  assert.notEqual(result.status, 0); assert.match(result.stderr, /No selected consumers/);
  assert(!result.stderr.includes('executablePath'));
});
test('a thrown invocation remains an executed input; module and endpoint failures stay separate', () => {
  const dir = mkdtempSync(join(tmpdir(), 'solid-witness-retry-')), selection = join(dir, 'selection.json'), browser = join(dir, 'browser.json');
  writeFileSync(selection, JSON.stringify({ rows: [{ package: 'test', export: 'candidate', arguments: ['1'],
    attempts: [{ arguments: ['1'], typingErrors: [] }, { arguments: ['2'], typingErrors: [] }] }] }));
  const makeRow = phase => ({ packagePins: [{ package: 'test' }], provenance: { export: 'candidate', phase },
    observations: [], errors: [{ label: 'invoke' }], pageErrors: [], consoleErrors: [], blockedRequests: [], publishedTypingErrors: [], feedback: [] });
  for (const [mode, expected] of [['throw', 'retry'], ['load', 'loader'], ['endpoint', 'endpoint']]) {
    const rows = ['unowned', 'owned'].map(makeRow);
    if (mode === 'load') for (const row of rows) { row.errors = [{ label: 'dispose' }]; row.consoleErrors = ['Failed to load resource: 500']; }
    if (mode === 'endpoint') for (const row of rows) { row.errors = []; row.observations = [{ label: 'invoke', ok: true }]; row.consoleErrors = ['Failed to load resource: 404']; }
    writeFileSync(browser, JSON.stringify({ finishedAt: 'test', results: rows })); const output = join(dir, mode + '.json');
    const result = spawnSync(process.execPath, [new URL('./argument-retry-selection.mjs', import.meta.url).pathname, selection, browser, output], { encoding: 'utf8' });
    assert.equal(result.status, 0, result.stderr);
    const report = JSON.parse(readFileSync(output, 'utf8'));
    if (expected === 'retry') { assert.deepEqual(report.rows[0].arguments, ['2']); assert.equal(report.exhausted.length, 0); }
    else { assert.equal(report.rows.length, 0); assert(report.exhausted[0].reason.includes(expected)); }
  }
});
