// Independently compare baseline result counts, selected assertions and source bytes.
import assert from 'node:assert/strict';
import {existsSync, readFileSync, writeFileSync} from 'node:fs';
import {join, relative, resolve} from 'node:path';
import {closurePins, hash} from './catalog.mjs';

const args = process.argv.slice(2), output = resolve(args.pop());
assert(args.length && !existsSync(output));
const reports = [];
for (const input of args) {
  const path = resolve(input), report = JSON.parse(readFileSync(path, 'utf8'));
  const rawPath = join(report.clone, '..', 'vitest.json'), raw = JSON.parse(readFileSync(rawPath, 'utf8'));
  assert.equal(report.authority, false);
  assert.equal(report.certification, false);
  assert.equal(report.preservedOriginal, true);
  assert.deepEqual(report.process, {exitCode: 0, signal: null});
  assert.equal(report.result.success, true);
  assert.equal(raw.success, true);
  const tests = raw.testResults.flatMap(suite => suite.assertionResults);
  assert(tests.length > 0, 'A baseline with no assertions is not a passing trial');
  assert(tests.every(test => test.status === 'passed'), 'Skipped or failed assertions remain open');
  assert.equal(raw.numTotalTests, tests.length);
  assert.equal(raw.numPassedTests, tests.length);
  assert.equal(raw.numFailedTests, 0);
  assert.equal(raw.numPendingTests, 0);
  assert.equal(report.result.tests, tests.length);
  assert.equal(report.result.passed, tests.length);
  assert.equal(report.result.failed, 0);
  assert.equal(report.result.pending, 0);
  assert.equal(hash(readFileSync(report.inputs.runner.path)), report.inputs.runner.sha256);
  const selected = report.command.slice(2).filter(arg => arg.startsWith('src/'));
  assert.equal(selected.length, new Set(selected).size);
  assert.deepEqual(raw.testResults.map(suite => relative(report.clone, suite.name)).sort(), [...selected].sort());
  for (const pin of report.inputs.application) {
    assert.equal(hash(readFileSync(join(report.source, pin.path))), pin.sha256, `Original input changed: ${pin.path}`);
    if (pin.path !== 'src/routeTree.gen.ts') assert.equal(hash(readFileSync(join(report.clone, pin.path))), pin.sha256, `Clone input changed: ${pin.path}`);
  }
  assert(report.cloneInputChanges.every(path => path === 'src/routeTree.gen.ts'));
  for (const pkg of report.inputs.packages) assert.deepEqual(closurePins(pkg.root), pkg.files, pkg.name);
  assert.equal(report.inputs.packages.find(pkg => pkg.name === 'solid-js').version, '2.0.0-rc.9');
  reports.push({input: {path, sha256: hash(readFileSync(path))}, raw: {path: rawPath, sha256: hash(readFileSync(rawPath))}, selection: report.selection, passedAssertions: tests.length,
    selectedFiles: selected, originalInputs: report.inputs.application.length, cloneInputChanges: report.cloneInputChanges, packages: report.inputs.packages.map(({name, version}) => ({name, version}))});
}
const audit = {authority: false, certification: false, auditedAt: new Date().toISOString(), reports,
  summary: {passedAssertions: reports.reduce((sum, report) => sum + report.passedAssertions, 0), files: reports.reduce((sum, report) => sum + report.selectedFiles.length, 0), failedAssertions: 0, skippedAssertions: 0},
  limits: ['No feedback instrumentation or precision measurement.', 'The original application configuration regenerated only the copied route tree.', 'Passing existing tests do not establish correctness for untested behavior or packages.']};
writeFileSync(output, JSON.stringify(audit, null, 2) + '\n');
console.log(JSON.stringify(audit.summary));
