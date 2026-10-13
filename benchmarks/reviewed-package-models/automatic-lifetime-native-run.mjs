// Focused original/native comparison; retain the rejected historical lowering.
import assert from 'node:assert/strict';
import { closeSync, existsSync, mkdirSync, openSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { hash } from './catalog.mjs';
import { ts } from './lower.mjs';
import cases from './automatic-lifetime-cases.mjs';
const [outputArg, browser] = process.argv.slice(2), output = resolve(outputArg); assert(browser && !existsSync(output)); mkdirSync(output, { recursive: true });
const files = new Set();
function walk(path) {
  path = resolve(path); if (files.has(path)) return; files.add(path); if (!path.endsWith('.mjs')) return;
  const source = ts.createSourceFile(path, readFileSync(path, 'utf8'), ts.ScriptTarget.Latest, true, ts.ScriptKind.JS);
  for (const node of source.statements) if ((ts.isImportDeclaration(node) || ts.isExportDeclaration(node)) && node.moduleSpecifier?.text?.startsWith('.')) walk(resolve(dirname(path), node.moduleSpecifier.text));
}
for (const name of ['automatic-lifetime-native-run.mjs', 'automatic-lifetime-transform.mjs', 'automatic-lifetime-runtime.mjs', 'automatic-lifetime-feedback.mjs', 'browser-experiment.mjs', 'guard-trace-runtime.mjs', 'origin-trace-runtime.mjs']) walk(new URL(name, import.meta.url).pathname);
const snapshot = () => [...files].sort().map(path => ({ path, sha256: hash(readFileSync(path)) })), before = snapshot(); mkdirSync(join(output, 'source-inputs'));
for (const file of before) writeFileSync(join(output, 'source-inputs', file.sha256.slice(7) + '.mjs'), readFileSync(file.path));
writeFileSync(join(output, 'inputs-before.json'), JSON.stringify({ authority: false, files: before }, null, 2) + '\n');
for (const profile of ['original', 'native']) {
  const log = openSync(join(output, profile + '.log'), 'wx'); let result;
  try { result = spawnSync(process.execPath, [new URL('./browser-experiment.mjs', import.meta.url).pathname, join(output, profile), browser, ['helge-app', ...cases.map(entry => entry.id)].join(',')], {
    env: { ...process.env, REVIEWED_MODEL_BROWSER_CASES: new URL('./automatic-lifetime-cases.mjs', import.meta.url).pathname,
      REVIEWED_MODEL_FEEDBACK_BRIDGE: new URL('./automatic-lifetime-feedback.mjs', import.meta.url).pathname,
      REVIEWED_MODEL_SOURCE_PLUGIN: profile === 'native' ? new URL('./automatic-lifetime-transform.mjs', import.meta.url).pathname : '',
      REVIEWED_MODEL_NATIVE_CONTEXT: profile === 'native' ? '1' : '', REVIEWED_MODEL_ASYNC_CONTEXT: '', REVIEWED_MODEL_ATTRIBUTION: '1',
      REVIEWED_MODEL_ATTRIBUTION_PREBUNDLE: '1', REVIEWED_MODEL_GUARD_TRACE: '', REVIEWED_MODEL_ORIGIN_TRACE: '' }, stdio: ['ignore', log, log], timeout: 180000,
  }); } finally { closeSync(log); }
  assert.equal(result.error, undefined); assert.equal(result.status, 0); assert.deepEqual(snapshot(), before);
  console.log(JSON.stringify({ profile, results: join(output, profile, 'results.json') }));
}
writeFileSync(join(output, 'inputs-after.json'), JSON.stringify({ authority: false, files: snapshot() }, null, 2) + '\n');
