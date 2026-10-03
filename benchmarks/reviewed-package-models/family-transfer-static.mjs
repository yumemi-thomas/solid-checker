// Independent source/native analysis with exact namespace import support.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { hash, read } from './catalog.mjs';
import { installedCatalog, demandSpecializer } from './demand-models.mjs';
import { lower, ts } from './lower.mjs';
import { projectFamilyWarnings } from './family-project-warning.mjs';
import { familyRequests } from './family-imports.mjs';
import { oracleCompilerOptions } from '../../scripts/tsc-oracle.mjs';
const [browserArg, outputArg] = process.argv.slice(2), browserPath = resolve(browserArg), output = resolve(outputArg), browser = read(browserPath);
assert(browser.finishedAt); assert(!existsSync(output)); mkdirSync(output, { recursive: true });
const checker = resolve('rust/target/debug/solid-checker-rust'), producer = resolve('bin/solid-typefacts');
const scriptPaths = ['family-transfer-static.mjs', 'family-imports.mjs', 'family-project-warning.mjs', 'demand-models.mjs', 'source-extractor.mjs', 'lower.mjs']
  .map(name => new URL(name, import.meta.url).pathname);
const results = [], report = { authority: false, certification: false, browserPath,
  inputs: [browserPath, checker, producer, ...scriptPaths].map(path => ({ path, sha256: hash(readFileSync(path)) })), results, finishedAt: null };
const save = () => writeFileSync(join(output, 'results.json'), JSON.stringify(report, null, 2) + '\n');
for (const row of browser.results) {
  const root = join(dirname(browserPath), row.id), path = join(root, 'src/main.tsx'); assert.equal(hash(readFileSync(path)), row.sourceSha256);
  const item = { id: row.id, sourceSha256: row.sourceSha256, refused: null, baseline: null, warnings: [], getter: null, classes: null }; results.push(item);
  if (row.publishedTypingErrors.length) { item.refused = 'TypeScript owns this input'; save(); continue; }
  const options = oracleCompilerOptions('v2', true, { customConditions: ['browser', 'development'] });
  const converted = ts.convertCompilerOptionsFromJson(options, root).options, program = ts.createProgram([path], converted), source = program.getSourceFile(path);
  assert.equal(ts.getPreEmitDiagnostics(program).filter(d => d.category === ts.DiagnosticCategory.Error).length, 0);
  const imports = familyRequests(program, source, row.packagePins.map(pin => pin.package));
  item.imports = imports; item.catalog = installedCatalog(root, imports.requests);
  const dir = join(output, row.id); mkdirSync(dir);
  function analyze(variant, main) {
    const config = join(dir, variant + '.json'); writeFileSync(config, JSON.stringify({ compilerOptions: options, files: [main] }));
    const result = spawnSync(checker, ['--format', 'json', '--runtime-target', 'browser', '--project', config],
      { env: { ...process.env, SOLID_TYPEFACTS_BIN: producer, SOLID_CHECKER_DAEMON: '0' }, encoding: 'utf8', timeout: 30000, maxBuffer: 32 * 1024 * 1024 });
    assert(!result.error, result.error?.message); assert([0, 1].includes(result.status), result.stderr); return JSON.parse(result.stdout);
  }
  item.baseline = analyze('baseline', path);
  if (!item.catalog.packages.some(p => p.error)) {
    const lowered = lower(program, source, item.catalog, 'browser', undefined, demandSpecializer(item.catalog, root));
    const modeled = join(root, 'src/family-modeled.tsx'); writeFileSync(modeled, lowered.text);
    item.sites = lowered.sites; item.unsupported = [...lowered.unsupported, ...imports.gaps];
    assert.equal(ts.getPreEmitDiagnostics(ts.createProgram([modeled], converted)).filter(d => d.category === ts.DiagnosticCategory.Error).length, 0);
    item.modeled = analyze('modeled', modeled);
    item.warnings = projectFamilyWarnings(item.modeled.findings, lowered, source, modeled, program);
  }
  save(); console.log(JSON.stringify({ id: row.id, imports: imports.requests, warnings: item.warnings.map(w => w.rule) }));
}
for (const pin of report.inputs) assert.equal(hash(readFileSync(pin.path)), pin.sha256);
report.finishedAt = new Date().toISOString(); save();
