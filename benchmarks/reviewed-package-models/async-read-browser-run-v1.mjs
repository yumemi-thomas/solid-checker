// Freeze this profile and consumers before executing narrow guard observations.
import assert from 'node:assert/strict';
import { closeSync, existsSync, mkdirSync, openSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { hash } from './catalog.mjs';
import { ts } from './lower.mjs';
const [casesArg, bridgeArg, outputArg, browser] = process.argv.slice(2), output = resolve(outputArg), casesPath = resolve(casesArg), bridgePath = resolve(bridgeArg);
assert(browser && !existsSync(output)); mkdirSync(output, { recursive: true });
const files = new Set();
function walk(path) {
  path = resolve(path); if (files.has(path)) return; files.add(path); if (!path.endsWith('.mjs')) return;
  const source = ts.createSourceFile(path, readFileSync(path, 'utf8'), ts.ScriptTarget.Latest, true, ts.ScriptKind.JS);
  for (const node of source.statements) if ((ts.isImportDeclaration(node) || ts.isExportDeclaration(node)) && node.moduleSpecifier?.text?.startsWith('.')) walk(resolve(dirname(path), node.moduleSpecifier.text));
}
for (const path of [casesPath, bridgePath, ...['async-read-browser-run-v1.mjs', 'snapshot-feedback-v2.mjs', 'browser-experiment-v4.mjs',
  'async-read-transform-v1.mjs', 'async-read-runtime-v1.mjs', 'family-matrix-feedback.mjs', 'guard-trace.mjs', 'guard-trace-runtime-v2.mjs', 'origin-trace-runtime.mjs'].map(name => new URL(name, import.meta.url).pathname)]) walk(path);
const snapshot = () => [...files].sort().map(path => ({ path, sha256: hash(readFileSync(path)) })), before = snapshot();
writeFileSync(join(output, 'inputs-before.json'), JSON.stringify({ authority: false, certification: false, files: before }, null, 2) + '\n');
const log = openSync(join(output, 'browser.log'), 'wx'); let result;
try { result = spawnSync(process.execPath, [new URL('./browser-experiment-v4.mjs', import.meta.url).pathname, join(output, 'browser'), browser], {
  env: { ...process.env, REVIEWED_MODEL_INPUT_SELECTION: '', REVIEWED_MODEL_BROWSER_CASES: casesPath,
    REVIEWED_MODEL_FEEDBACK_BRIDGE: bridgePath,
    REVIEWED_MODEL_ATTRIBUTION: '1', REVIEWED_MODEL_ATTRIBUTION_PREBUNDLE: '', REVIEWED_MODEL_SOURCE_PLUGIN: new URL('./async-read-transform-v1.mjs', import.meta.url).pathname,
    REVIEWED_MODEL_GUARD_TRACE: '1', REVIEWED_MODEL_ORIGIN_TRACE: '' }, stdio: ['ignore', log, log], timeout: 360000,
}); } finally { closeSync(log); }
assert.equal(result.error, undefined); assert.equal(result.status, 0); assert.deepEqual(snapshot(), before);
writeFileSync(join(output, 'inputs-after.json'), JSON.stringify({ authority: false, certification: false, files: snapshot() }, null, 2) + '\n');
console.log(JSON.stringify({ frozenFiles: before.length, results: join(output, 'browser/results.json') }));
