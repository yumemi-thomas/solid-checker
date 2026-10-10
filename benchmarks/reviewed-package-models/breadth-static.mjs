// Fresh static observations for the same consumers that actually ran. The
// existing extractor/lowerer stay frozen; unsupported behavior is retained.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { hash, read } from "./catalog.mjs";
import { installedCatalog, demandSpecializer } from "./demand-models.mjs";
import { lower, projectWarning, ts } from "./lower.mjs";
import { oracleCompilerOptions } from "../../scripts/tsc-oracle.mjs";
const [browserArgument, outArgument, filter = ''] = process.argv.slice(2), browserPath = resolve(browserArgument), out = resolve(outArgument);
assert(!existsSync(out)); mkdirSync(out, { recursive: true });
const browser = read(browserPath); assert(browser.finishedAt);
const checker = resolve('rust/target/release/solid-checker-rust'), producer = resolve('bin/solid-typefacts');
const studyPath = resolve(process.env.REVIEWED_MODEL_BREADTH_STUDY ?? 'rust/target/reviewed-models-breadth-study.json');
const frozen = read(studyPath).frozen;
for (const input of frozen) assert.equal(hash(readFileSync(new URL(input.name, import.meta.url))), input.digest);
const report = { authority: false, studyPath, browserPath, browserSha256: hash(readFileSync(browserPath)), frozen, typescript: ts.version,
  checkerSha256: hash(readFileSync(checker)), producerSha256: hash(readFileSync(producer)), results: [], finishedAt: null };
const save = () => writeFileSync(join(out, 'results.json'), JSON.stringify(report, null, 2) + '\n');
const issueRules = family => family === 'callback-phase' ? ['reactive-write-in-owned-scope'] :
  ['async-owner-loss', 'listener-lifetime', 'delayed-callback-lifetime'].includes(family) ? ['missing-owner', 'leaf-owner-forbidden-call'] :
  ['context', 'component-provider'].includes(family) ? ['missing-owner', 'primitive-in-forbidden-scope'] : ['strict-read-untracked'];
for (const row of browser.results.filter(row => !filter || filter.split(',').includes(row.id))) {
  const root = join(dirname(browserPath), row.id), main = join(root, row.mountResultExposedForDisposal ? 'src/index.tsx' : 'src/main.tsx');
  assert.equal(hash(readFileSync(main)), row.sourceSha256);
  const item = { id: row.id, provenance: row.provenance, expectedIssueRules: issueRules(row.provenance.family), refused: null, sources: [], baseline: null, modeled: null, warnings: [] };
  report.results.push(item);
  if (row.publishedTypingErrors.length) { item.refused = 'published typings rejected consumer'; save(); continue; }
  if (row.runtime.some(runtime => runtime.version !== '2.0.0-rc.9')) { item.refused = 'installed runtime outside frozen rc.9 vocabulary'; save(); continue; }
  const options = { ...oracleCompilerOptions('v2', true, { customConditions: ['browser', 'development'] }), allowJs: true };
  const converted = ts.convertCompilerOptionsFromJson(options, root).options, ambient = join(root, 'src/vite-env.d.ts');
  const program = ts.createProgram([main, ...(existsSync(ambient) ? [ambient] : [])], converted);
  const errors = ts.getPreEmitDiagnostics(program).filter(d => d.category === ts.DiagnosticCategory.Error);
  assert.equal(errors.length, 0);
  const sources = program.getSourceFiles().filter(source => source.fileName.startsWith(join(root, 'src') + '/') && !source.isDeclarationFile);
  const requests = sources.flatMap(source => source.statements.flatMap(statement => {
    if (!ts.isImportDeclaration(statement) || !ts.isStringLiteral(statement.moduleSpecifier) || !statement.importClause) return [];
    const name = statement.moduleSpecifier.text;
    if (!row.packagePins.some(input => input.package === name)) return [];
    const exports = [], clause = statement.importClause;
    if (clause.name) exports.push('default');
    if (clause.namedBindings && ts.isNamedImports(clause.namedBindings)) for (const specifier of clause.namedBindings.elements) exports.push((specifier.propertyName ?? specifier.name).text);
    return [{ package: name, exports }];
  }));
  const catalog = installedCatalog(root, requests); item.catalog = catalog;
  if (catalog.packages.some(pkg => pkg.error)) { item.refused = 'installed catalog input refused'; save(); continue; }
  const specialize = demandSpecializer(catalog, root), mappings = [];
  for (const source of sources) {
    const lowered = lower(program, source, catalog, 'browser', undefined, specialize), modeled = source.fileName.replace(/\.(tsx?|jsx?)$/, '-breadth-modeled.$1');
    mappings.push({ source, lowered, modeled });
    item.sources.push({ path: source.fileName, sha256: hash(source.text), modeled, modeledSha256: hash(lowered.text), sites: lowered.sites, unsupported: lowered.unsupported });
  }
  const dir = join(out, row.id); mkdirSync(dir);
  function analyze(variant, files) {
    const project = join(dir, variant + '.json'); writeFileSync(project, JSON.stringify({ compilerOptions: options, files }));
    const result = spawnSync(checker, ['--format', 'json', '--runtime-target', 'browser', '--project', project], { encoding: 'utf8', timeout: 30000,
      maxBuffer: 32 * 1024 * 1024, env: { ...process.env, SOLID_TYPEFACTS_BIN: producer, SOLID_CHECKER_DAEMON: '0' } });
    assert([0, 1].includes(result.status), result.stderr); const output = JSON.parse(result.stdout);
    writeFileSync(join(dir, variant + '-output.json'), JSON.stringify(output, null, 2) + '\n'); return output;
  }
  // Do not accidentally analyze generated twins in the baseline app graph.
  item.baseline = analyze('baseline', [main]);
  // Each experimental twin is its own project root. Cross-file app semantics
  // still refer to the original imports; these outputs do not claim a fully
  // rewritten app graph. Core baseline findings remain the app observation.
  for (const mapping of mappings) writeFileSync(mapping.modeled, mapping.lowered.text);
  item.analysisTwinTypingErrors = [];
  for (const mapping of mappings) {
    const twin = ts.createProgram([mapping.modeled, ...(existsSync(ambient) ? [ambient] : [])], converted);
    const diagnostics = ts.getPreEmitDiagnostics(twin).filter(d => d.category === ts.DiagnosticCategory.Error);
    item.analysisTwinTypingErrors.push(...diagnostics.map(d => ({ code: d.code, message: ts.flattenDiagnosticMessageText(d.messageText, '\n') })));
  }
  assert.equal(item.analysisTwinTypingErrors.length, 0);
  item.modeled = analyze('modeled', mappings.map(mapping => mapping.modeled));
  for (const finding of item.modeled.findings) for (const mapping of mappings) {
    const warning = projectWarning(finding, mapping.lowered, mapping.source, mapping.modeled); if (warning) item.warnings.push(warning);
  }
  item.baselineRelevantViolations = item.baseline.findings.filter(f => f.kind === 'violation' && item.expectedIssueRules.includes(f.rule));
  item.modelRelevantWarnings = item.warnings.filter(w => item.expectedIssueRules.includes(w.rule));
  save(); console.log(`${row.id}: baseline=${item.baselineRelevantViolations.length}, modeled=${item.modelRelevantWarnings.length}, premises=${item.sources.flatMap(s => s.sites).filter(s => s.applied).length}`);
}
report.finishedAt = new Date().toISOString(); save();
console.log(JSON.stringify({ observations: report.results.length, refused: report.results.filter(row => row.refused).length,
  targetObservations: report.results.filter(row => row.provenance.expectedIssue && !row.refused).length,
  baselineDetectedTargets: report.results.filter(row => row.provenance.expectedIssue && row.baselineRelevantViolations?.length).length,
  modelDetectedTargets: report.results.filter(row => row.provenance.expectedIssue && row.modelRelevantWarnings?.length).length,
  controlsWithRelevantFindings: report.results.filter(row => !row.provenance.expectedIssue && (row.baselineRelevantViolations?.length || row.modelRelevantWarnings?.length)).map(row => row.id) }));
