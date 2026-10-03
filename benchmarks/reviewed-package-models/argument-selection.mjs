import assert from 'node:assert/strict';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { authenticateModel, hash, read } from './catalog.mjs';
import { ts } from './lower.mjs';
import { seeds, synthesize } from './argument-witnesses.mjs';
import { oracleCompilerOptions } from '../../scripts/tsc-oracle.mjs';
const [runArg, catalogArg, outArg] = process.argv.slice(2), run = read(resolve(runArg)), catalog = read(resolve(catalogArg)), output = resolve(outArg);
assert(!existsSync(output)); const rows = [], refusals = [], zeroArgumentExports = [];
for (const model of catalog.models) {
  const names = Object.entries(model.exports).filter(([, b]) => b.browser?.owner).map(([name]) => name); if (!names.length) continue;
  const project = run.results.find(row => row.package === model.package).retainedArtifacts.projectDir;
  authenticateModel(model, project);
  const path = join(project, 'src/__argument_selection.tsx');
  const imports = names.map((name, i) => `import { ${JSON.stringify(name)} as candidate${i} } from ${JSON.stringify(model.package)};`).join('\n');
  const seedSource = seeds.map((seed, i) => `const seed${i} = ${seed};`).join('\n');
  const options = ts.convertCompilerOptionsFromJson({ ...oracleCompilerOptions('v2', true, { customConditions: ['browser', 'development'] }), jsxImportSource: '@solidjs/web' }, dirname(path)).options;
  function programFor(code) {
    const host = ts.createCompilerHost(options), get = host.getSourceFile, exists = host.fileExists;
    host.getSourceFile = (file, ...args) => file === path ? ts.createSourceFile(path, code, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX) : get(file, ...args);
    host.fileExists = file => file === path || exists(file);
    return ts.createProgram([path], options, host);
  }
  const original = programFor(imports + '\n' + seedSource), source = original.getSourceFile(path), checker = original.getTypeChecker();
  const seedTypes = source.statements.slice(names.length).map(s => checker.getTypeAtLocation(s.declarationList.declarations[0].name));
  const pending = [], probes = [];
  for (let i = 0; i < names.length; i++) {
    const local = source.statements[i].importClause.namedBindings.elements[0].name, symbol = checker.getSymbolAtLocation(local);
    const target = symbol?.flags & ts.SymbolFlags.Alias ? checker.getAliasedSymbol(symbol) : symbol;
    const signatures = checker.getTypeAtLocation(local).getCallSignatures();
    if (signatures.some(s => s.minArgumentCount === 0)) { zeroArgumentExports.push({ package: model.package, export: names[i] }); continue; }
    const declaration = target?.declarations?.map(node => ({ path: node.getSourceFile().fileName, start: node.getStart(), sha256: hash(node.getSourceFile().text) })) ?? [];
    const candidates = [], seen = new Set();
    if (declaration.length) for (const signature of signatures.slice(0, 4)) for (let profile = 0; profile < 3; profile++) {
      if (signature.minArgumentCount > 5) continue;
      const args = signature.getParameters().slice(0, signature.minArgumentCount).map(p => synthesize(checker, checker.getTypeOfSymbolAtLocation(p, local), local, seedTypes, profile));
      if (args.some(a => a === null)) continue;
      const key = JSON.stringify(args); if (seen.has(key)) continue; seen.add(key);
      const id = probes.length, text = `function probe${id}() { candidate${i}(${args.join(', ')}); }`;
      probes.push(text); candidates.push({ id, arguments: args, signature: checker.signatureToString(signature), profile });
    }
    pending.push({ package: model.package, version: model.version, project, export: names[i], declaration,
      sourcePremise: model.exports[names[i]].browser, pins: model.pins, catalogSourceReferences: model.sourceReferences, candidates });
  }
  if (!pending.length) continue;
  const checked = programFor(imports + '\n' + probes.join('\n')), checkedSource = checked.getSourceFile(path), errors = ts.getPreEmitDiagnostics(checked).filter(d => d.category === ts.DiagnosticCategory.Error);
  const starts = checkedSource.statements.slice(names.length).map(s => [s.getStart(), s.end]);
  const unrelated = errors.filter(e => e.file?.fileName !== path || !starts.some(([start, end]) => e.start >= start && e.start < end));
  for (const row of pending) {
    const attempts = row.candidates.map(c => ({ ...c, typingErrors: errors.filter(e => e.file?.fileName === path && e.start >= starts[c.id][0] && e.start < starts[c.id][1]).map(e => ({ code: e.code, message: ts.flattenDiagnosticMessageText(e.messageText, '\n') })) }));
    const admitted = !unrelated.length && attempts.find(a => !a.typingErrors.length);
    const { candidates, ...base } = row;
    if (admitted) rows.push({ ...base, arguments: admitted.arguments, chosenSignature: admitted.signature, profile: admitted.profile, attempts });
    else refusals.push({ ...base, reason: unrelated.length ? 'Published dependency typing errors' : attempts.length ? 'No synthesized call passes published strict typings' : 'No bounded type witness', attempts,
      unrelatedTypingErrors: unrelated.map(e => ({ code: e.code, message: ts.flattenDiagnosticMessageText(e.messageText, '\n') })) });
  }
}
const report = { authority: false, certification: false, basis: 'bounded type witnesses under a positive source owner assumption',
  catalogPath: resolve(catalogArg), catalogSha256: hash(readFileSync(resolve(catalogArg))), typescript: ts.version, rows, refusals, zeroArgumentExports,
  summary: { argumentBearingExports: rows.length + refusals.length, selectedExports: rows.length, selectedPackages: new Set(rows.map(r => r.package)).size,
    refusals: refusals.length, zeroArgumentExports: zeroArgumentExports.length } };
writeFileSync(output, JSON.stringify(report, null, 2) + '\n'); console.log(JSON.stringify(report.summary, null, 2));
