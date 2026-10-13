// Independent source/compiler observations for the exact executed consumers.
// Consumer expectations are not inputs to analysis or warning selection.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { hash, read } from './catalog.mjs';
import { installedCatalog, demandSpecializer } from './demand-models.mjs';
import { lower, projectWarning, ts } from './lower.mjs';
import { getterConsumerFlows } from './getter-paths.mjs';
import { classSnapshotFlows } from './class-footprints.mjs';
import { oracleCompilerOptions } from '../../scripts/tsc-oracle.mjs';
const [selectionArg, browserArg, outputArg] = process.argv.slice(2);
const selectionPath = resolve(selectionArg), browserPath = resolve(browserArg), output = resolve(outputArg);
assert(!existsSync(output)); mkdirSync(output, { recursive: true });
const selection = read(selectionPath), browser = read(browserPath); assert(browser.finishedAt);
for (const input of selection.frozen) assert.equal(hash(readFileSync(input.path)), input.sha256);
const checker = resolve('rust/target/debug/solid-checker-rust'), producer = resolve('bin/solid-typefacts');
const results = [], report = { authority: false, certification: false, inputs: [selectionPath, browserPath, checker, producer]
  .map(path => ({ path, sha256: hash(readFileSync(path)) })), results, finishedAt: null };
const save = () => writeFileSync(join(output, 'results.json'), JSON.stringify(report, null, 2) + '\n');
for (const row of browser.results) {
  const root = join(dirname(browserPath), row.id), path = join(root, 'src/main.tsx');
  assert.equal(hash(readFileSync(path)), row.sourceSha256);
  const item = { id: row.id, sourceSha256: row.sourceSha256, refused: null, baseline: null, warnings: [], getter: null, classes: null }; results.push(item);
  if (row.publishedTypingErrors.length) { item.refused = 'TypeScript owns this input'; save(); continue; }
  const options = oracleCompilerOptions('v2', true, { customConditions: ['browser', 'development'] });
  const program = ts.createProgram([path], ts.convertCompilerOptionsFromJson(options, root).options), source = program.getSourceFile(path);
  assert.equal(ts.getPreEmitDiagnostics(program).filter(d => d.category === ts.DiagnosticCategory.Error).length, 0);
  const requests = source.statements.flatMap(statement => {
    if (!ts.isImportDeclaration(statement) || !row.packagePins.some(p => p.package === statement.moduleSpecifier.text)) return [];
    const binding = statement.importClause?.namedBindings;
    const exports = binding && ts.isNamedImports(binding) ? binding.elements.filter(e => !e.isTypeOnly).map(e => (e.propertyName ?? e.name).text) : [];
    return exports.length ? [{ package: statement.moduleSpecifier.text, exports }] : [];
  });
  item.catalog = installedCatalog(root, requests);
  item.getter = getterConsumerFlows(program, source);
  item.classes = classSnapshotFlows(program, source);
  const dir = join(output, row.id); mkdirSync(dir);
  function analyze(variant, main) {
    const config = join(dir, variant + '.json'); writeFileSync(config, JSON.stringify({ compilerOptions: options, files: [main] }));
    const result = spawnSync(checker, ['--format', 'json', '--runtime-target', 'browser', '--project', config],
      { env: { ...process.env, SOLID_TYPEFACTS_BIN: producer, SOLID_CHECKER_DAEMON: '0' }, encoding: 'utf8', timeout: 30000, maxBuffer: 32 * 1024 * 1024 });
    assert(!result.error, result.error?.message); assert([0, 1].includes(result.status), result.stderr);
    return JSON.parse(result.stdout);
  }
  item.baseline = analyze('baseline', path);
  if (!item.catalog.packages.some(p => p.error)) {
    const lowered = lower(program, source, item.catalog, 'browser', undefined, demandSpecializer(item.catalog, root));
    const modeled = join(root, 'src/family-modeled.tsx'); writeFileSync(modeled, lowered.text);
    item.sites = lowered.sites; item.unsupported = lowered.unsupported;
    const typing = ts.getPreEmitDiagnostics(ts.createProgram([modeled], ts.convertCompilerOptionsFromJson(options, root).options)).filter(d => d.category === ts.DiagnosticCategory.Error);
    assert.equal(typing.length, 0, 'Source analysis twin must pass real published typings');
    item.modeled = analyze('modeled', modeled);
    item.warnings = item.modeled.findings.map(f => projectWarning(f, lowered, source, modeled)).filter(Boolean);
  }
  save(); console.log(`${row.id}: native=${item.baseline.findings.length}, source=${item.warnings.length}, getter=${item.getter.candidates?.length ?? 0}`);
}
for (const input of selection.frozen) assert.equal(hash(readFileSync(input.path)), input.sha256);
report.finishedAt = new Date().toISOString(); save();
