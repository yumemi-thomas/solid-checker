// Measure whole-project feedback rather than one analyzer process per specimen.
import assert from 'node:assert/strict';
import { existsSync, mkdirSync, readFileSync, symlinkSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { performance } from 'node:perf_hooks';
import { authenticateModel, hash, nativeRuntimeRoots, packageRoot, read } from './catalog.mjs';
import { lower, projectWarning, ts } from './lower.mjs';
import { oracleCompilerOptions } from '../../scripts/tsc-oracle.mjs';
const [selectionArg, outputArg] = process.argv.slice(2), selection = read(resolve(selectionArg)), catalog = read(selection.catalogPath), output = resolve(outputArg);
assert(!existsSync(output)); mkdirSync(output, { recursive: true }); const project = selection.rows[0].project;
const modules = join(output, 'node_modules'); mkdirSync(modules);
const link = (name, root) => { const target = join(modules, name); mkdirSync(dirname(target), { recursive: true }); if (!existsSync(target)) symlinkSync(root, target, 'dir'); };
for (const runtime of nativeRuntimeRoots(project)) link(read(join(runtime, 'package.json')).name, runtime);
for (const row of selection.rows) link(row.package, packageRoot(row.project, row.package));
for (const model of catalog.models.filter(model => selection.rows.some(row => row.package === model.package))) authenticateModel(model, output);
const checker = resolve(process.env.SOLID_CHECKER_NATIVE_BIN ?? 'rust/target/release/solid-checker-rust'), typefacts = resolve(process.env.SOLID_TYPEFACTS_BIN ?? 'bin/solid-typefacts');
const originalPath = join(output, 'original.tsx'), modeledPath = join(output, 'modeled.tsx');
const code = [`import { createRoot } from 'solid-js';`, ...selection.rows.map((row, i) => `import { ${JSON.stringify(row.export)} as candidate${i} } from ${JSON.stringify(row.package)};`),
  ...selection.rows.flatMap((row, i) => [`export const unowned${i} = candidate${i}();`, `export const owned${i} = createRoot(() => { const result = candidate${i}(); return result; });`])].join('\n');
writeFileSync(originalPath, code);
const options = oracleCompilerOptions('v2', true, { customConditions: ['browser', 'development'] }), converted = ts.convertCompilerOptionsFromJson(options, output).options;
const prepare = performance.now(), program = ts.createProgram([originalPath], converted), source = program.getSourceFile(originalPath);
assert.equal(ts.getPreEmitDiagnostics(program).filter(d => d.category === ts.DiagnosticCategory.Error).length, 0);
const mapping = lower(program, source, catalog, 'browser'); writeFileSync(modeledPath, mapping.text);
assert.equal(ts.getPreEmitDiagnostics(ts.createProgram([modeledPath], converted)).filter(d => d.category === ts.DiagnosticCategory.Error).length, 0);
const preparationMs = performance.now() - prepare, observations = {};
for (const [variant, path] of [['baseline', originalPath], ['modeled', modeledPath]]) {
  const config = join(output, variant + '.json'); writeFileSync(config, JSON.stringify({ compilerOptions: options, files: [path] }));
  const started = performance.now(), result = spawnSync(checker, ['--format', 'json', '--runtime-target', 'browser', '--project', config], {
    env: { ...process.env, SOLID_TYPEFACTS_BIN: typefacts, SOLID_CHECKER_DAEMON: '0' }, encoding: 'utf8', timeout: 30000, maxBuffer: 16 * 1024 * 1024,
  }); assert.equal(result.error, undefined); assert([0, 1].includes(result.status), result.stderr);
  observations[variant] = { ...JSON.parse(result.stdout), durationMs: performance.now() - started };
  writeFileSync(join(output, variant + '-output.json'), JSON.stringify(observations[variant], null, 2) + '\n');
}
const warnings = observations.modeled.findings.map(finding => projectWarning(finding, mapping, source, modeledPath)).filter(Boolean);
const rows = selection.rows.map(row => ({ package: row.package, export: row.export, warnings: warnings.filter(w => w.package === row.package && w.export === row.export) }));
for (const row of rows) assert.equal(row.warnings.filter(w => w.rule === 'missing-owner').length, 1, row.export);
assert.equal(warnings.length, selection.rows.length); assert(mapping.sites.every(site => site.applied && site.behavior.owner));
const report = { authority: false, certification: false, consumerCalls: selection.rows.length * 2, packages: new Set(selection.rows.map(row => row.package)).size,
  targetsDetected: rows.length, ownedControlsWithWarnings: 0, preparationMs, baselineMs: observations.baseline.durationMs, modeledMs: observations.modeled.durationMs,
  sourceSha256: hash(code), modelSha256: hash(readFileSync(selection.catalogPath)), checkerSha256: hash(readFileSync(checker)), typefactsSha256: hash(readFileSync(typefacts)),
  scope: 'one synthetic type-only project linking separately retained installs; no runtime execution, editor latency or large-app benchmark', rows, unsupported: mapping.unsupported };
writeFileSync(join(output, 'mapping.json'), JSON.stringify(mapping, null, 2) + '\n'); writeFileSync(join(output, 'results.json'), JSON.stringify(report, null, 2) + '\n');
console.log(JSON.stringify({ calls: report.consumerCalls, packages: report.packages, detected: report.targetsDetected, controlWarnings: 0, preparationMs, baselineMs: report.baselineMs, modeledMs: report.modeledMs }, null, 2));
