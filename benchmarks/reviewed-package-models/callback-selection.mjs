// Published-type sampling across every retained package root, including roots
// without a positive owner/accessor source model.
import assert from 'node:assert/strict';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { closurePins, hash, nativeRuntimeRoots, packageRoot, read } from './catalog.mjs';
import { ts } from './lower.mjs';
import { seeds, synthesize } from './argument-witnesses.mjs';
import { CallbackPaths, callbackArguments } from './callback-paths.mjs';
import { runtimeEntry } from './source-extractor.mjs';
import { oracleCompilerOptions } from '../../scripts/tsc-oracle.mjs';
const [runArg, outputArg] = process.argv.slice(2), run = read(resolve(runArg)), output = resolve(outputArg); assert(!existsSync(output));
const rows = [], refusals = [], packages = [], models = [];
for (const retained of run.results) {
  const name = retained.package, project = retained.retainedArtifacts.projectDir, item = { package: name, exports: 0, candidates: 0, selected: 0, error: null }; packages.push(item);
  try {
    for (const root of nativeRuntimeRoots(project)) assert.equal(read(join(root, 'package.json')).version, '2.0.0-rc.9');
    const root = packageRoot(project, name), manifest = read(join(root, 'package.json')), pins = closurePins(root);
    for (const pin of pins.filter(p => ['solid-js', '@solidjs/signals', '@solidjs/web'].includes(p.package))) assert.equal(pin.version, '2.0.0-rc.9');
    const path = join(project, 'src/__callback_selection.tsx');
    const options = ts.convertCompilerOptionsFromJson({ ...oracleCompilerOptions('v2', true, { customConditions: ['browser', 'development'] }), jsxImportSource: '@solidjs/web' }, dirname(path)).options;
    function programFor(code) {
      const host = ts.createCompilerHost(options), get = host.getSourceFile, exists = host.fileExists;
      host.getSourceFile = (file, ...args) => file === path ? ts.createSourceFile(path, code, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX) : get(file, ...args);
      host.fileExists = file => file === path || exists(file); return ts.createProgram([path], options, host);
    }
    const imports = `import * as Package from ${JSON.stringify(name)};`;
    const original = programFor(imports + '\n' + seeds.map((seed, i) => `const seed${i} = ${seed};`).join('\n')), source = original.getSourceFile(path), checker = original.getTypeChecker();
    const moduleSymbol = checker.getSymbolAtLocation(source.statements[0].moduleSpecifier); assert(moduleSymbol, 'Published root types unresolved');
    const exports = checker.getExportsOfModule(moduleSymbol); item.exports = exports.length;
    const seedTypes = source.statements.slice(1).map(s => checker.getTypeAtLocation(s.declarationList.declarations[0].name));
    const pending = [], probes = [];
    function callable(type, depth = 0, seen = new Set()) {
      if (depth > 2 || seen.has(type)) return false; const next = new Set(seen); next.add(type);
      if (type.getCallSignatures().length) return true;
      if (type.isUnion()) return type.types.some(t => callable(t, depth + 1, next));
      return checker.getPropertiesOfType(type).slice(0, 8).some(p => callable(checker.getTypeOfSymbolAtLocation(p, source), depth + 1, next));
    }
    for (const exported of exports) {
      const target = exported.flags & ts.SymbolFlags.Alias ? checker.getAliasedSymbol(exported) : exported;
      const signatures = checker.getTypeOfSymbolAtLocation(target, source).getCallSignatures();
      if (!signatures.length) continue;
      const declaration = target.declarations?.map(d => ({ path: d.getSourceFile().fileName, start: d.getStart(), sha256: hash(d.getSourceFile().text) })) ?? [];
      if (!declaration.length) continue;
      const candidates = [], seen = new Set();
      for (const signature of signatures.slice(0, 4)) {
        const params = signature.getParameters(), callbackIndices = params.map((p, i) => callable(checker.getTypeOfSymbolAtLocation(p, source)) ? i : -1).filter(i => i >= 0);
        if (!callbackIndices.length) continue;
        const count = Math.max(signature.minArgumentCount, Math.max(...callbackIndices) + 1); if (count > 5) continue;
        for (let profile = 0; profile < 3; profile++) {
          const args = params.slice(0, count).map(p => synthesize(checker, checker.getTypeOfSymbolAtLocation(p, source), source, seedTypes, profile));
          if (args.some(a => a === null)) continue; const key = JSON.stringify(args); if (seen.has(key)) continue; seen.add(key);
          const parsed = ts.createSourceFile('probe.ts', `candidate(${args.join(', ')});`, ts.ScriptTarget.Latest, true);
          const converted = callbackArguments(parsed.statements[0].expression.arguments); if (!converted.callbacks.length) continue;
          const id = probes.length; probes.push(`function probe${id}() { Package[${JSON.stringify(exported.getName())}](${args.join(', ')}); }`);
          candidates.push({ id, arguments: args, profile, callbacks: converted.callbacks, signature: checker.signatureToString(signature) });
        }
      }
      if (candidates.length) pending.push({ package: name, version: manifest.version, project, export: exported.getName(), declaration, pins, candidates });
    }
    item.candidates = pending.length;
    const checked = programFor(imports + '\n' + probes.join('\n')), checkedSource = checked.getSourceFile(path), errors = ts.getPreEmitDiagnostics(checked).filter(d => d.category === ts.DiagnosticCategory.Error);
    const starts = checkedSource.statements.slice(1).map(s => [s.getStart(), s.end]);
    const unrelated = errors.filter(e => e.file?.fileName !== path || !starts.some(([start, end]) => e.start >= start && e.start < end));
    let entry, entryError; try { entry = runtimeEntry(join(root, 'package.json'), name, 'browser'); } catch (error) { entryError = error.message; }
    const engine = new CallbackPaths('browser');
    for (const row of pending) {
      const attempts = row.candidates.map(c => ({ ...c, typingErrors: errors.filter(e => e.file?.fileName === path && e.start >= starts[c.id][0] && e.start < starts[c.id][1]).map(e => ({ code: e.code, message: ts.flattenDiagnosticMessageText(e.messageText, '\n') })) }));
      const admitted = !unrelated.length && attempts.find(a => !a.typingErrors.length), { candidates, ...base } = row;
      if (!admitted) { refusals.push({ package: name, export: row.export, reason: unrelated.length ? 'Published dependency typing errors' : 'No type-valid callback witness', attempts }); continue; }
      const parsed = ts.createSourceFile('probe.ts', `candidate(${admitted.arguments.join(', ')});`, ts.ScriptTarget.Latest, true), args = callbackArguments(parsed.statements[0].expression.arguments);
      const call = checkedSource.statements[admitted.id + 1].body.statements[0].expression, finalChecker = checked.getTypeChecker(), returnType = finalChecker.getTypeAtLocation(call);
      const acceptsNoArgs = type => !!type && type.getCallSignatures().some(s => s.minArgumentCount === 0);
      const consume = acceptsNoArgs(returnType) ? 'self' : finalChecker.isTupleType(returnType) && acceptsNoArgs(finalChecker.getTypeArguments(returnType)[0]) ? 'tuple0' : null;
      let callbackSource; try { callbackSource = entry ? engine.profile(entry, row.export, args.values) : { assumptions: [], footprints: [], gaps: [entryError] }; }
      catch (error) { callbackSource = { assumptions: [], footprints: [], gaps: [error.message] }; }
      rows.push({ ...base, ...admitted, attempts, callbackSource, consume }); item.selected++;
    }
    models.push({ package: name, version: manifest.version, pins, exports: {}, sourceReferences: [...engine.modules.values()].map(m => ({ path: m.path, sha256: hash(m.source.text) })) });
    assert.deepEqual(closurePins(root), pins);
  } catch (error) { item.error = error.message; }
}
const catalogPath = output.replace(/\.json$/, '-catalog.json'); assert(!existsSync(catalogPath));
writeFileSync(catalogPath, JSON.stringify({ format: 'solid-checker-reviewed-model-experiment', authority: false, models }, null, 2) + '\n');
const report = { authority: false, certification: false, basis: 'published callback types and bounded source context assumptions', catalogPath,
  catalogSha256: hash(readFileSync(catalogPath)), selectionProducer: new URL('./callback-selection.mjs', import.meta.url).pathname,
  rows, refusals, packages, summary: { roots: packages.length, exports: packages.reduce((n,p) => n+p.exports,0), candidates: packages.reduce((n,p)=>n+p.candidates,0),
    admittedExports: rows.length, admittedPackages: new Set(rows.map(r=>r.package)).size, rootsRefused: packages.filter(p=>p.error).length,
    sourceAssumptions: rows.filter(r=>r.callbackSource.assumptions.length).length, sourceFootprints: rows.filter(r=>r.callbackSource.footprints.length).length } };
writeFileSync(output, JSON.stringify(report, null, 2) + '\n'); console.log(JSON.stringify(report.summary, null, 2));
