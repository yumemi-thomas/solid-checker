// Freeze the experiment's local source closure and selection before execution.
import assert from 'node:assert/strict';
import { closeSync, existsSync, mkdirSync, openSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { hash, read } from './catalog.mjs';
import { ts } from './lower.mjs';
const [casesArg, bridgeArg, outputArg, browser, selectionArg] = process.argv.slice(2), output = resolve(outputArg), casesPath = resolve(casesArg), bridgePath = resolve(bridgeArg);
assert(browser && !existsSync(output)); mkdirSync(output, { recursive: true }); const files = new Set();
function walk(path) {
  path = resolve(path); if (files.has(path)) return; files.add(path); if (!path.endsWith('.mjs')) return;
  const source = ts.createSourceFile(path, readFileSync(path, 'utf8'), ts.ScriptTarget.Latest, true, ts.ScriptKind.JS);
  for (const node of source.statements) if ((ts.isImportDeclaration(node) || ts.isExportDeclaration(node)) && node.moduleSpecifier?.text?.startsWith('.')) walk(resolve(dirname(path), node.moduleSpecifier.text));
}
for (const path of [new URL('./observation-browser-run.mjs', import.meta.url).pathname, casesPath, bridgePath,
  ...['browser-experiment.mjs', 'guard-trace-runtime.mjs', 'origin-trace-runtime.mjs'].map(name => new URL(name, import.meta.url).pathname)]) walk(path);
if (selectionArg) { walk(selectionArg); const selection = read(resolve(selectionArg)); assert(selection.rows.length, 'No selected consumers; refusing the harness default-case fallback');
  walk(selection.catalogPath); walk(new URL('./argument-selection.mjs', import.meta.url).pathname); walk(new URL('./argument-retry-selection.mjs', import.meta.url).pathname);
  if (selection.parentSelectionPath) { assert.equal(hash(readFileSync(selection.parentSelectionPath)), selection.parentSelectionSha256); walk(selection.parentSelectionPath); }
  if (selection.parentBrowserPath) { assert.equal(hash(readFileSync(selection.parentBrowserPath)), selection.parentBrowserSha256); walk(selection.parentBrowserPath); }
  if (selection.historyPath) { assert.equal(hash(readFileSync(selection.historyPath)), selection.historySha256); walk(selection.historyPath); }
  if (selection.selectionProducer) walk(selection.selectionProducer);
}
if (process.env.REVIEWED_MODEL_SOURCE_PLUGIN) walk(process.env.REVIEWED_MODEL_SOURCE_PLUGIN);
const snapshot = () => [...files].sort().map(path => ({ path, sha256: hash(readFileSync(path)) })), before = snapshot(); mkdirSync(join(output, 'source-inputs'));
for (const file of before) writeFileSync(join(output, 'source-inputs', file.sha256.slice(7)), readFileSync(file.path));
writeFileSync(join(output, 'inputs-before.json'), JSON.stringify({ authority: false, files: before, selectionPath: selectionArg ? resolve(selectionArg) : null }, null, 2) + '\n');
const log = openSync(join(output, 'browser.log'), 'wx'); let result;
try { result = spawnSync(process.execPath, [new URL('./browser-experiment.mjs', import.meta.url).pathname, join(output, 'browser'), browser], {
  env: { ...process.env, REVIEWED_MODEL_INPUT_SELECTION: selectionArg ? resolve(selectionArg) : '', REVIEWED_MODEL_BROWSER_CASES: casesPath,
    REVIEWED_MODEL_FEEDBACK_BRIDGE: bridgePath, REVIEWED_MODEL_ATTRIBUTION: '1', REVIEWED_MODEL_ATTRIBUTION_PREBUNDLE: '1',
    REVIEWED_MODEL_GUARD_TRACE: '', REVIEWED_MODEL_ORIGIN_TRACE: '' }, stdio: ['ignore', log, log], timeout: 360000,
}); } finally { closeSync(log); }
assert.equal(result.error, undefined); assert.equal(result.status, 0); assert.deepEqual(snapshot(), before);
writeFileSync(join(output, 'inputs-after.json'), JSON.stringify({ authority: false, files: snapshot() }, null, 2) + '\n');
console.log(JSON.stringify({ frozenFiles: before.length, results: join(output, 'browser/results.json') }));
