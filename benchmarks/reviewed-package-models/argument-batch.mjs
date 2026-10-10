// Type-only project scale test. Separately retained runtime graphs are linked
// for declaration resolution; this does not assert runtime graph equivalence.
import assert from 'node:assert/strict';
import { existsSync, mkdirSync, readFileSync, symlinkSync, writeFileSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { performance } from 'node:perf_hooks';
import { authenticateModel, hash, nativeRuntimeRoots, packageRoot, read } from './catalog.mjs';
import { lower, projectWarning, ts } from './lower.mjs';
import { demandSpecializer, installedCatalog } from './demand-models.mjs';
import { oracleCompilerOptions } from '../../scripts/tsc-oracle.mjs';
const [selectionArg, outputArg, profile = 'base'] = process.argv.slice(2), selection = read(resolve(selectionArg)), catalog = read(selection.catalogPath), output = resolve(outputArg);
assert(['base', 'demand'].includes(profile)); assert(!existsSync(output)); mkdirSync(output, { recursive: true });
const modules = join(output, 'node_modules'); mkdirSync(modules);
const link = (name, root) => { const target = join(modules, name); mkdirSync(dirname(target), { recursive: true }); if (!existsSync(target)) symlinkSync(root, target, 'dir'); };
for (const runtime of nativeRuntimeRoots(selection.rows[0].project)) link(read(join(runtime, 'package.json')).name, runtime);
for (const row of selection.rows) link(row.package, packageRoot(row.project, row.package));
for (const model of catalog.models.filter(model => selection.rows.some(row => row.package === model.package))) authenticateModel(model, output);
const checker = resolve(process.env.SOLID_CHECKER_NATIVE_BIN ?? 'rust/target/release/solid-checker-rust'), typefacts = resolve(process.env.SOLID_TYPEFACTS_BIN ?? 'bin/solid-typefacts');
const originalPath = join(output, 'original.tsx'), modeledPath = join(output, 'modeled.tsx');
const code = [`import { createRoot } from 'solid-js';`, ...selection.rows.map((row, i) => `import { ${JSON.stringify(row.export)} as candidate${i} } from ${JSON.stringify(row.package)};`),
  ...selection.rows.flatMap((row, i) => [`candidate${i}(${row.arguments.join(', ')});`, `createRoot(() => { candidate${i}(${row.arguments.join(', ')}); });`])].join('\n');
writeFileSync(originalPath, code);
const options = oracleCompilerOptions('v2', true, { customConditions: ['browser', 'development'] }), converted = ts.convertCompilerOptionsFromJson(options, output).options;
const prepare = performance.now(), program = ts.createProgram([originalPath], converted), source = program.getSourceFile(originalPath);
const errors = ts.getPreEmitDiagnostics(program).filter(d => d.category === ts.DiagnosticCategory.Error); assert.equal(errors.length, 0, ts.formatDiagnosticsWithColorAndContext(errors, { getCanonicalFileName: p => p, getCurrentDirectory: () => output, getNewLine: () => '\n' }));
const requests = [...new Set(selection.rows.map(row => row.package))].map(name => ({ package: name, exports: selection.rows.filter(row => row.package === name).map(row => row.export) }));
let localCatalog = profile === 'demand' ? installedCatalog(output, requests, 'browser') : catalog;
// TS can redirect a same-version dependency declaration to a different
// retained physical installation. Keep the exact-artifact admission boundary:
// that import is unmodeled, rather than trusting the package ID redirect.
const declarationRefusals = [], typeChecker = program.getTypeChecker();
for (const statement of source.statements) {
  if (!ts.isImportDeclaration(statement) || statement.moduleSpecifier.text === 'solid-js') continue;
  const name = statement.moduleSpecifier.text, model = localCatalog.models.find(m => m.package === name); if (!model) continue;
  const root = authenticateModel(model, output), local = statement.importClause.namedBindings.elements[0].name;
  const symbol = typeChecker.getSymbolAtLocation(local), target = symbol.flags & ts.SymbolFlags.Alias ? typeChecker.getAliasedSymbol(symbol) : symbol;
  const outside = target.declarations.filter(d => { const path = relative(root, d.getSourceFile().fileName); return !path || path.startsWith('..') || path.startsWith('/'); });
  if (outside.length) declarationRefusals.push({ package: name, export: statement.importClause.namedBindings.elements[0].propertyName.text,
    reason: 'Resolved declaration outside the authenticated package artifact', root, declarations: outside.map(d => d.getSourceFile().fileName) });
}
localCatalog = { ...localCatalog, models: localCatalog.models.filter(m => !declarationRefusals.some(r => r.package === m.package)) };
const specialize = profile === 'demand' ? demandSpecializer(localCatalog, output) : null;
const mapping = lower(program, source, localCatalog, 'browser', undefined, specialize); writeFileSync(modeledPath, mapping.text);
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
const rows = selection.rows.map((row, i) => {
  const sites = mapping.sites.filter(site => site.package === row.package && site.export === row.export);
  if (declarationRefusals.some(r => r.package === row.package)) { assert.equal(sites.length, 0); return { package: row.package, export: row.export, arguments: row.arguments,
    sites: ['unowned', 'owned'].map(role => ({ role, applied: false, warnings: [], refusal: 'Package declaration identity refused' })) }; }
  assert.equal(sites.length, 2);
  const observed = sites.map((site, index) => ({ role: index ? 'owned' : 'unowned', applied: !!site.applied, behavior: site.behavior,
    warnings: warnings.filter(w => w.location.startByte >= Buffer.byteLength(code.slice(0, site.start)) && w.location.startByte < Buffer.byteLength(code.slice(0, site.end))) }));
  assert.equal(observed[1].warnings.length, 0, row.export);
  return { package: row.package, export: row.export, arguments: row.arguments, sites: observed };
});
const report = { authority: false, certification: false, profile, selectionPath: resolve(selectionArg), selectionSha256: hash(readFileSync(resolve(selectionArg))),
  consumerCalls: selection.rows.length * 2, packages: new Set(selection.rows.map(row => row.package)).size,
  targetsDetected: rows.filter(row => row.sites[0].warnings.some(w => w.rule === 'missing-owner')).length,
  ownerPremisesApplied: rows.flatMap(row => row.sites).filter(site => site.applied && site.behavior?.owner).length,
  ownedControlsWithWarnings: 0, originalTypingErrors: 0, modeledTypingErrors: 0,
  preparationMs, baselineMs: observations.baseline.durationMs, modeledMs: observations.modeled.durationMs,
  sourceSha256: hash(code), modelSha256: hash(readFileSync(selection.catalogPath)), checkerSha256: hash(readFileSync(checker)), typefactsSha256: hash(readFileSync(typefacts)),
  scope: 'one synthetic type-only project linking separately retained installs; no runtime execution, editor latency or large-app benchmark', rows, unsupported: mapping.unsupported, declarationRefusals,
  ...(specialize ? { specializationStats: specialize.stats, localCatalog } : {}) };
writeFileSync(join(output, 'mapping.json'), JSON.stringify(mapping, null, 2) + '\n'); writeFileSync(join(output, 'results.json'), JSON.stringify(report, null, 2) + '\n');
console.log(JSON.stringify({ profile, calls: report.consumerCalls, detected: report.targetsDetected, applied: report.ownerPremisesApplied, controlWarnings: 0, preparationMs, baselineMs: report.baselineMs, modeledMs: report.modeledMs }, null, 2));
