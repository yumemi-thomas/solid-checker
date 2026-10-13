import assert from 'node:assert/strict';
import { closeSync, existsSync, mkdirSync, openSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { hash, read } from './catalog.mjs';
import { ts } from './lower.mjs';
const [selectionArg, outputArg, browser] = process.argv.slice(2), output = resolve(outputArg), selectionPath = resolve(selectionArg), selection = read(selectionPath);
assert(browser && !existsSync(output)); mkdirSync(output, { recursive: true }); const files = new Set();
function walk(path) {
  path = resolve(path); if (files.has(path)) return; files.add(path); if (!path.endsWith('.mjs')) return;
  const source = ts.createSourceFile(path, readFileSync(path, 'utf8'), ts.ScriptTarget.Latest, true, ts.ScriptKind.JS);
  for (const node of source.statements) if ((ts.isImportDeclaration(node) || ts.isExportDeclaration(node)) && node.moduleSpecifier?.text?.startsWith('.')) walk(resolve(dirname(path), node.moduleSpecifier.text));
}
for (const name of ['zero-argument-run.mjs', 'zero-argument-cases.mjs', 'zero-argument-selection.mjs', 'extract-catalog.mjs', 'browser-experiment.mjs', 'extended-runtime-feedback.mjs', 'guard-trace-runtime.mjs', 'origin-trace-runtime.mjs']) walk(new URL(name, import.meta.url).pathname);
walk(selectionPath); walk(selection.catalogPath);
const snapshot = () => [...files].sort().map(path => ({ path, sha256: hash(readFileSync(path)) })), before = snapshot(); mkdirSync(join(output, 'source-inputs'));
for (const file of before) writeFileSync(join(output, 'source-inputs', file.sha256.slice(7)), readFileSync(file.path));
writeFileSync(join(output, 'inputs-before.json'), JSON.stringify({ authority: false, files: before, selectionPath }, null, 2) + '\n');
const log = openSync(join(output, 'browser.log'), 'wx'); let result;
try { result = spawnSync(process.execPath, [new URL('./browser-experiment.mjs', import.meta.url).pathname, join(output, 'browser'), browser], {
  env: { ...process.env, REVIEWED_MODEL_ZERO_ARGUMENT_SELECTION: selectionPath, REVIEWED_MODEL_BROWSER_CASES: new URL('./zero-argument-cases.mjs', import.meta.url).pathname,
    REVIEWED_MODEL_FEEDBACK_BRIDGE: new URL('./extended-runtime-feedback.mjs', import.meta.url).pathname, REVIEWED_MODEL_SOURCE_PLUGIN: '',
    REVIEWED_MODEL_ATTRIBUTION: '1', REVIEWED_MODEL_ATTRIBUTION_PREBUNDLE: '1', REVIEWED_MODEL_GUARD_TRACE: '', REVIEWED_MODEL_ORIGIN_TRACE: '' },
  stdio: ['ignore', log, log], timeout: 180000,
}); } finally { closeSync(log); }
assert.equal(result.error, undefined); assert.equal(result.status, 0); assert.deepEqual(snapshot(), before);
writeFileSync(join(output, 'inputs-after.json'), JSON.stringify({ authority: false, files: snapshot() }, null, 2) + '\n');
console.log(JSON.stringify({ frozenFiles: before.length, results: join(output, 'browser/results.json') }));
