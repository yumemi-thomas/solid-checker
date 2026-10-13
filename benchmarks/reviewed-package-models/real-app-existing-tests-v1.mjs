// Run existing application assertions unchanged in an isolated source copy.
// A passing baseline is not a feedback precision result or package certificate.
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {closeSync, copyFileSync, existsSync, mkdirSync, openSync, readFileSync, readdirSync, realpathSync, symlinkSync, writeFileSync} from 'node:fs';
import {dirname, join, relative, resolve} from 'node:path';
import {performance} from 'node:perf_hooks';
import {fileURLToPath} from 'node:url';
import {closurePins, hash, packageRoot} from './catalog.mjs';

const selections = {
  ui: [
    'src/ui/kobalte/__tests__/Dialog.test.tsx',
    'src/ui/kobalte/__tests__/Select.test.tsx',
    'src/ui/virtual/__tests__/VirtualList.test.tsx',
    'src/ui/__tests__/composites.test.tsx',
  ],
  relay: [
    'src/relay/__tests__/network.test.ts',
    'src/relay/__tests__/connections.test.ts',
    'src/relay/__tests__/ssr-isolation.test.tsx',
    'src/relay/__tests__/document.test.tsx',
  ],
};
const [outArg, selection = 'ui'] = process.argv.slice(2);
assert(outArg && Object.hasOwn(selections, selection));
const output = resolve(outArg), source = resolve('rust/target/app-import-metric/apps/finds-team/frontend');
assert(!existsSync(output), 'Use a fresh output directory');
const excluded = new Set(['node_modules', '.git', '.tanstack', '.vinxi', 'build', 'dist', 'coverage', 'test-results', 'playwright-report']);
function applicationPins(root) {
  const files = [];
  function visit(folder) {
    for (const entry of readdirSync(folder, {withFileTypes: true})) {
      if (excluded.has(entry.name) || entry.name.startsWith('.env')) continue;
      const path = join(folder, entry.name);
      if (entry.isDirectory()) visit(path);
      else if (entry.isFile()) files.push({path: relative(root, path), sha256: hash(readFileSync(path))});
      else assert.fail(`Unsupported application input: ${relative(root, path)}`);
    }
  }
  visit(root);
  return files.sort((a, b) => a.path.localeCompare(b.path));
}
const appPins = applicationPins(source), clone = join(output, 'application');
for (const test of selections[selection]) assert(appPins.some(pin => pin.path === test), test);
const packages = ['vitest', 'vite', 'vite-plugin-solid', 'vite-plugin-cjs-interop', 'vite-plugin-relay-lite', '@tanstack/solid-start', '@tanstack/solid-router', '@tanstack/virtual-core', 'relay-runtime', 'solid-js', '@solidjs/web', '@solidjs/signals', 'jsdom', 'tabbable'];
const packagePins = packages.map(name => {
  const root = packageRoot(name === '@solidjs/signals' ? packageRoot(source, 'solid-js') : source, name);
  return {name, root, version: JSON.parse(readFileSync(join(root, 'package.json'))).version, files: closurePins(root)};
});
mkdirSync(clone, {recursive: true});
for (const pin of appPins) {
  const target = join(clone, pin.path);
  mkdirSync(dirname(target), {recursive: true});
  copyFileSync(join(source, pin.path), target);
  assert.equal(hash(readFileSync(target)), pin.sha256);
}
symlinkSync(realpathSync(join(source, 'node_modules')), join(clone, 'node_modules'), 'dir');
const resultPath = join(output, 'vitest.json'), logPath = join(output, 'vitest.log');
const command = [join(packageRoot(source, 'vitest'), 'vitest.mjs'), 'run', ...selections[selection], '--no-file-parallelism', '--maxWorkers=1', '--reporter=json', `--outputFile=${resultPath}`];
const report = {
  authority: false, certification: false, selection,
  scope: 'Unchanged existing tests and original Vite configuration in an isolated copy; no feedback instrumentation, seeded defect, install, or application fix.',
  startedAt: new Date().toISOString(), node: process.version, source, clone, command,
  inputs: {runner: {path: fileURLToPath(import.meta.url), sha256: hash(readFileSync(fileURLToPath(import.meta.url)))}, application: appPins, packages: packagePins},
  preservedOriginal: null, result: null,
};
const save = () => writeFileSync(join(output, 'results.json'), JSON.stringify(report, null, 2) + '\n');
save();
const log = openSync(logPath, 'wx'), start = performance.now();
try {
  const child = spawn(process.execPath, command, {cwd: clone, stdio: ['ignore', log, log], timeout: 120000});
  report.process = await new Promise(resolveProcess => {
    child.once('error', error => resolveProcess({error: {message: error.message, code: error.code}}));
    child.once('close', (code, signal) => resolveProcess({exitCode: code, signal}));
  });
} finally { closeSync(log); }
report.elapsedMs = performance.now() - start;
if (existsSync(resultPath)) {
  const result = JSON.parse(readFileSync(resultPath, 'utf8'));
  report.result = {
    success: result.success, tests: result.numTotalTests, passed: result.numPassedTests,
    failed: result.numFailedTests, pending: result.numPendingTests,
    suites: (result.testResults ?? []).map(suite => ({name: relative(clone, suite.name), status: suite.status, message: suite.message,
      assertions: (suite.assertionResults ?? []).map(test => ({name: test.fullName, status: test.status, failureMessages: test.failureMessages}))})),
  };
}
assert.deepEqual(applicationPins(source), appPins, 'Original application inputs changed');
for (const pkg of packagePins) assert.deepEqual(closurePins(pkg.root), pkg.files, pkg.name);
report.preservedOriginal = true;
report.cloneInputChanges = appPins.filter(pin => hash(readFileSync(join(clone, pin.path))) !== pin.sha256).map(pin => pin.path);
report.finishedAt = new Date().toISOString();
save();
console.log(JSON.stringify({selection, process: report.process, result: report.result && {success: report.result.success, tests: report.result.tests, passed: report.result.passed, failed: report.result.failed, pending: report.result.pending}, preservedOriginal: report.preservedOriginal, cloneInputChanges: report.cloneInputChanges}));
