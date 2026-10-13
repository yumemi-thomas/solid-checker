// Recheck changed causal propagation with inputs frozen before and after launch.
import assert from 'node:assert/strict';
import { closeSync, existsSync, mkdirSync, openSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { hash } from './catalog.mjs';
import { ts } from './lower.mjs';
const [outputArg, browser] = process.argv.slice(2), output = resolve(outputArg); assert(browser && !existsSync(output)); mkdirSync(output, { recursive: true });
const files = new Set();
function walk(path) {
  path = resolve(path); if (files.has(path)) return; files.add(path);
  if (!path.endsWith('.mjs')) return;
  const source = ts.createSourceFile(path, readFileSync(path, 'utf8'), ts.ScriptTarget.Latest, true, ts.ScriptKind.JS); assert.deepEqual(source.parseDiagnostics, []);
  for (const statement of source.statements) if ((ts.isImportDeclaration(statement) || ts.isExportDeclaration(statement)) && statement.moduleSpecifier?.text?.startsWith('.')) walk(resolve(dirname(path), statement.moduleSpecifier.text));
}
for (const name of ['lifetime-final-run.mjs', 'browser-experiment.mjs', 'lifetime-all-cases.mjs', 'lifetime-feedback.mjs', 'guard-trace-runtime.mjs', 'origin-trace-runtime.mjs']) walk(new URL(name, import.meta.url).pathname);
const snapshot = () => [...files].sort().map(path => ({ path, sha256: hash(readFileSync(path)) })), before = snapshot();
writeFileSync(join(output, 'inputs-before.json'), JSON.stringify({ authority: false, files: before, startedAt: new Date().toISOString(), typescript: ts.version }, null, 2) + '\n');
const log = openSync(join(output, 'browser.log'), 'wx');
let result;
try { result = spawnSync(process.execPath, [new URL('./browser-experiment.mjs', import.meta.url).pathname, join(output, 'browser'), browser], {
  env: { ...process.env, REVIEWED_MODEL_BROWSER_CASES: new URL('./lifetime-all-cases.mjs', import.meta.url).pathname,
    REVIEWED_MODEL_FEEDBACK_BRIDGE: new URL('./lifetime-feedback.mjs', import.meta.url).pathname, REVIEWED_MODEL_ATTRIBUTION: '1', REVIEWED_MODEL_ATTRIBUTION_PREBUNDLE: '1' },
  stdio: ['ignore', log, log], timeout: 180000,
}); } finally { closeSync(log); }
assert.equal(result.error, undefined); assert.equal(result.status, 0); assert.deepEqual(snapshot(), before, 'Experimental inputs changed during browser execution');
writeFileSync(join(output, 'inputs-after.json'), JSON.stringify({ authority: false, files: snapshot(), finishedAt: new Date().toISOString(), status: result.status }, null, 2) + '\n');
console.log(JSON.stringify({ results: join(output, 'browser/results.json'), frozenFiles: before.length, inputsUnchanged: true }));
