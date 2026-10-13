// Re-evaluate real recorded guards against a virtual next source revision.
// The actual consumer files and retained observations are never modified.
import assert from 'node:assert/strict';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { read, hash, closurePins, packageRoot } from './catalog.mjs';
import { ts } from './lower.mjs';
import { getterSnapshotsV11 } from './snapshot-feedback-v11.mjs';
import { getterSnapshotsV12 } from './snapshot-feedback-v12.mjs';
import { oracleCompilerOptions } from '../../scripts/tsc-oracle.mjs';
const [browserArg, outputArg] = process.argv.slice(2), browserPath = resolve(browserArg), output = resolve(outputArg),
  browser = read(browserPath), profile = dirname(dirname(browserPath)), before = read(join(profile, 'inputs-before.json')), after = read(join(profile, 'inputs-after.json'));
assert(!existsSync(output)); assert(browser.finishedAt); assert.equal(browser.authority, false); assert.deepEqual(before.files, after.files);
for (const pin of before.files) assert.equal(hash(readFileSync(pin.path)), pin.sha256);
const modules = ['snapshot-feedback-v11.mjs', 'snapshot-feedback-v12.mjs', 'local-call-targets-v1.mjs'].map(name => new URL(name, import.meta.url).pathname),
  detectorInputs = modules.map(path => ({ path, sha256: hash(readFileSync(path)) })), results = [];
for (const row of browser.results) {
  const root = join(dirname(browserPath), row.id), path = join(root, 'src/main.tsx'), original = readFileSync(path, 'utf8');
  assert.equal(hash(original), row.sourceSha256); assert.equal(row.publishedTypingErrors.length, 0); assert.equal(row.harnessFailure ?? null, null);
  for (const pkg of row.packagePins) assert.deepEqual(closurePins(packageRoot(root, pkg.package)), pkg.pins);
  assert(row.guardTrace.length); assert(row.guardTrace.every(event => Array.isArray(event.originalFrames)));
  const variants = [];
  for (const changed of [false, true]) {
    const text = original + (changed ? '\n// virtual next source revision\n' : ''), options = ts.convertCompilerOptionsFromJson({ ...oracleCompilerOptions('v2', true,
      { customConditions: ['browser', 'development'] }), allowJs: true }, root).options, host = ts.createCompilerHost(options), load = host.getSourceFile.bind(host);
    host.getSourceFile = (file, languageVersion, onError, shouldCreateNewSourceFile) => file === path ? ts.createSourceFile(file, text, languageVersion, true, ts.ScriptKind.TSX)
      : load(file, languageVersion, onError, shouldCreateNewSourceFile);
    const program = ts.createProgram([path], options, host), source = program.getSourceFile(path);
    assert.equal(ts.getPreEmitDiagnostics(program).filter(d => d.category === ts.DiagnosticCategory.Error).length, 0);
    const earlier = getterSnapshotsV11(program, source, row.guardTrace), current = getterSnapshotsV12(program, source, row.guardTrace);
    variants.push({ changed, sourceSha256: hash(text), earlierHints: earlier.candidates.length, currentHints: current.candidates.length,
      currentOpenReasons: current.open.map(item => item.reason) });
    if (changed) assert.equal(current.candidates.length, 0);
  }
  assert.equal(hash(readFileSync(path)), row.sourceSha256);
  results.push({ id: row.id, role: row.provenance.role, variants });
}
const target = results.find(row => row.role === 'target'), control = results.find(row => row.role === 'control'); assert(target && control);
assert.equal(target.variants[0].currentHints, 1); assert.equal(target.variants[1].earlierHints, 1);
assert.equal(control.variants[0].currentHints, 0); assert.equal(control.variants[1].currentHints, 0);
for (const pin of detectorInputs) assert.equal(hash(readFileSync(pin.path)), pin.sha256);
writeFileSync(output, JSON.stringify({ authority: false, certification: false, finishedAt: new Date().toISOString(),
  inputs: [{ path: browserPath, sha256: hash(readFileSync(browserPath)) }], detectorInputs, sourceFilesUnchanged: true,
  checks: ['authentic actual caller observations', 'unchanged real source files and package closures', 'published typing validation of both virtual revisions',
    'valid observations retain target feedback', 'stale observations refused', 'explicit intentional control quiet'], results,
  limit: 'This tests the observation/source boundary. It does not measure a complete hot-reload or editor integration.' }, null, 2) + '\n');
console.log(JSON.stringify({ target: target.variants, control: control.variants, output }));
