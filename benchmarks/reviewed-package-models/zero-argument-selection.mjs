// Select only source-premised exports whose actual published declarations admit
// a zero-argument call. This generates observations, never accepted contracts.
import assert from 'node:assert/strict';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { authenticateModel, hash, read } from './catalog.mjs';
import { ts } from './lower.mjs';
import { oracleCompilerOptions } from '../../scripts/tsc-oracle.mjs';
const [runArg, catalogArg, outArg] = process.argv.slice(2), run = read(resolve(runArg)), catalog = read(resolve(catalogArg)), output = resolve(outArg);
assert(!existsSync(output));
const rows = [], refusals = [];
for (const model of catalog.models) {
  const names = Object.entries(model.exports).filter(([, behavior]) => behavior.browser?.owner).map(([name]) => name); if (!names.length) continue;
  const retained = run.results.find(row => row.package === model.package), project = retained.retainedArtifacts.projectDir;
  try {
    authenticateModel(model, project);
    const path = join(project, 'src/__zero_argument_selection.tsx'), code = names.map((name, i) => `import { ${JSON.stringify(name)} as candidate${i} } from ${JSON.stringify(model.package)};`).join('\n');
    const options = ts.convertCompilerOptionsFromJson({ ...oracleCompilerOptions('v2', true, { customConditions: ['browser', 'development'] }), jsxImportSource: '@solidjs/web' }, dirname(path)).options;
    const host = ts.createCompilerHost(options), get = host.getSourceFile, exists = host.fileExists;
    host.getSourceFile = (file, ...args) => file === path ? ts.createSourceFile(path, code, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX) : get(file, ...args);
    host.fileExists = file => file === path || exists(file);
    const program = ts.createProgram([path], options, host), source = program.getSourceFile(path), checker = program.getTypeChecker();
    for (let i = 0; i < names.length; i++) {
      const local = source.statements[i].importClause.namedBindings.elements[0].name, symbol = checker.getSymbolAtLocation(local);
      const target = symbol?.flags & ts.SymbolFlags.Alias ? checker.getAliasedSymbol(symbol) : symbol;
      const signatures = checker.getTypeAtLocation(local).getCallSignatures();
      const admitted = signatures.filter(signature => signature.minArgumentCount === 0);
      const declaration = target?.declarations?.map(node => ({ path: node.getSourceFile().fileName, start: node.getStart(), sha256: hash(node.getSourceFile().text) })) ?? [];
      if (!admitted.length || !declaration.length) { refusals.push({ package: model.package, export: names[i], reason: admitted.length ? 'Exact declaration unavailable' : 'No published zero-argument call signature' }); continue; }
      rows.push({ package: model.package, version: model.version, project, export: names[i], declaration, sourcePremise: model.exports[names[i]].browser,
        pins: model.pins, zeroArgumentSignatures: admitted.map(signature => checker.signatureToString(signature)), catalogSourceReferences: model.sourceReferences });
    }
  } catch (error) { refusals.push({ package: model.package, exports: names, reason: error.message }); }
}
const report = { authority: false, certification: false, basis: 'published zero-argument call with positive source owner assumption',
  catalogPath: resolve(catalogArg), catalogSha256: hash(readFileSync(resolve(catalogArg))), rows, refusals,
  summary: { sourceOwnerExports: catalog.models.reduce((sum, model) => sum + Object.values(model.exports).filter(b => b.browser?.owner).length, 0),
    selectedExports: rows.length, selectedPackages: new Set(rows.map(row => row.package)).size, refusals: refusals.length } };
writeFileSync(output, JSON.stringify(report, null, 2) + '\n'); console.log(JSON.stringify(report.summary, null, 2));
