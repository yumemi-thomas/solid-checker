// Enumerate the actual imported value surface, including export-equals members.
// This driver admits published types and finite consumers, without contracting
// arbitrary member behavior or guessing dispatch from a spelling.
import assert from 'node:assert/strict';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { closurePins, hash, nativeRuntimeRoots, packageDigest, packageRoot, read } from './catalog.mjs';
import { ts } from './lower.mjs';
import { extendedWitness, standardSeeds } from './extended-witnesses.mjs';
import { CallbackPaths, callbackArguments } from './callback-paths.mjs';
import { runtimeEntry } from './source-extractor.mjs';
import { oracleCompilerOptions } from '../../scripts/tsc-oracle.mjs';
const [runArg, outputArg, namesArg] = process.argv.slice(2), runPath = resolve(runArg), run = read(runPath), output = resolve(outputArg);
assert(!existsSync(output)); const names = namesArg ? new Set(namesArg.split(',')) : null;
const rows = [], refusals = [], packages = [], models = [], typingCache = new Map();
function typingRoot(path) {
  for (let dir = dirname(path); ; dir = dirname(dir)) {
    if (existsSync(join(dir, 'package.json'))) {
      if (!typingCache.has(dir)) { const manifest = read(join(dir, 'package.json')); typingCache.set(dir,
        { root: dir, package: manifest.name, version: manifest.version, digest: packageDigest(dir) }); }
      return typingCache.get(dir);
    }
    if (dirname(dir) === dir) throw new Error('Declaration package root unavailable');
  }
}
for (const retained of run.results.filter(r => !names || names.has(r.package))) {
  const name = retained.package, project = retained.retainedArtifacts.projectDir;
  const item = { package: name, members: 0, callableMembers: 0, candidates: 0, selected: 0, error: null }; packages.push(item);
  try {
    for (const root of nativeRuntimeRoots(project)) assert.equal(read(join(root, 'package.json')).version, '2.0.0-rc.9');
    const root = packageRoot(project, name), manifest = read(join(root, 'package.json')), pins = closurePins(root);
    for (const pin of pins.filter(p => ['solid-js', '@solidjs/signals', '@solidjs/web'].includes(p.package))) assert.equal(pin.version, '2.0.0-rc.9');
    const path = join(project, 'src/__value_surface.tsx');
    const options = ts.convertCompilerOptionsFromJson({ ...oracleCompilerOptions('v2', true, { customConditions: ['browser', 'development'] }), jsxImportSource: '@solidjs/web' }, dirname(path)).options;
    function programFor(code) {
      const host = ts.createCompilerHost(options), get = host.getSourceFile, exists = host.fileExists;
      host.getSourceFile = (file, ...args) => file === path ? ts.createSourceFile(path, code, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX) : get(file, ...args);
      host.fileExists = file => file === path || exists(file); return ts.createProgram([path], options, host);
    }
    const initial = programFor(`import * as Package from ${JSON.stringify(name)};`), initialSource = initial.getSourceFile(path);
    const module = initial.getTypeChecker().getSymbolAtLocation(initialSource.statements[0].moduleSpecifier); assert(module, 'Published root types unresolved');
    const importStyle = module.exports?.has(ts.InternalSymbolName.ExportEquals) ? 'default' : 'namespace'; item.importStyle = importStyle;
    const imports = importStyle === 'default' ? `import Package from ${JSON.stringify(name)};` : `import * as Package from ${JSON.stringify(name)};`;
    const original = programFor(imports + '\nconst surface = Package;\n' + standardSeeds.map((seed, i) => `const seed${i} = ${seed};`).join('\n'));
    const source = original.getSourceFile(path), checker = original.getTypeChecker();
    const surface = checker.getTypeAtLocation(source.statements[1].declarationList.declarations[0].name), members = checker.getPropertiesOfType(surface);
    item.members = members.length;
    const seedTypes = source.statements.slice(2).map(s => checker.getTypeAtLocation(s.declarationList.declarations[0].name));
    const pending = [], probes = [];
    function callable(type, depth = 0, seen = new Set()) {
      if (depth > 2 || seen.has(type)) return false; const next = new Set(seen); next.add(type);
      if (type.getCallSignatures().length) return true;
      if (type.isUnion()) return type.types.some(t => callable(t, depth + 1, next));
      return checker.getPropertiesOfType(type).slice(0, 8).some(p => callable(checker.getTypeOfSymbolAtLocation(p, source), depth + 1, next));
    }
    for (const member of members) {
      const target = member.flags & ts.SymbolFlags.Alias ? checker.getAliasedSymbol(member) : member;
      const signatures = checker.getTypeOfSymbolAtLocation(target, source).getCallSignatures(); if (!signatures.length) continue;
      item.callableMembers++;
      const declaration = target.declarations?.map(d => ({ path: d.getSourceFile().fileName, start: d.getStart(), sha256: hash(d.getSourceFile().text) })) ?? [];
      if (!declaration.length) continue;
      const typingPins = [...new Map(declaration.map(d => { const pin = typingRoot(d.path); return [pin.root, pin]; })).values()];
      const candidates = [], seen = new Set();
      for (const signature of signatures.slice(0, 4)) {
        const params = signature.getParameters(), callbackIndices = params.map((p, i) => callable(checker.getTypeOfSymbolAtLocation(p, source)) ? i : -1).filter(i => i >= 0);
        if (!callbackIndices.length) continue;
        const count = Math.max(signature.minArgumentCount, Math.max(...callbackIndices) + 1); if (count > 5) continue;
        for (const profile of [1, 0, 2]) {
          const args = params.slice(0, count).map(p => extendedWitness(checker, checker.getTypeOfSymbolAtLocation(p, source), source, seedTypes, profile));
          if (args.some(a => a === null)) continue; const key = JSON.stringify(args); if (seen.has(key)) continue; seen.add(key);
          const parsed = ts.createSourceFile('probe.ts', `candidate(${args.join(', ')});`, ts.ScriptTarget.Latest, true);
          const converted = callbackArguments(parsed.statements[0].expression.arguments); if (!converted.callbacks.length) continue;
          const id = probes.length; probes.push(`function probe${id}() { Package[${JSON.stringify(member.getName())}](${args.join(', ')}); }`);
          candidates.push({ id, arguments: args, profile, callbacks: converted.callbacks, signature: checker.signatureToString(signature) });
        }
      }
      if (candidates.length) pending.push({ package: name, version: manifest.version, project, export: member.getName(), importStyle,
        declaration, typingPins, pins, candidates });
    }
    item.candidates = pending.length;
    const checked = programFor(imports + '\n' + probes.join('\n')), checkedSource = checked.getSourceFile(path);
    const errors = ts.getPreEmitDiagnostics(checked).filter(d => d.category === ts.DiagnosticCategory.Error), starts = checkedSource.statements.slice(1).map(s => [s.getStart(), s.end]);
    const unrelated = errors.filter(e => e.file?.fileName !== path || !starts.some(([start, end]) => e.start >= start && e.start < end));
    const engine = new CallbackPaths('browser'); let entry, entryError;
    try { entry = runtimeEntry(join(root, 'package.json'), name, 'browser'); } catch (error) { entryError = error.message; }
    for (const row of pending) {
      const attempts = row.candidates.map(c => ({ ...c, typingErrors: errors.filter(e => e.file?.fileName === path && e.start >= starts[c.id][0] && e.start < starts[c.id][1])
        .map(e => ({ code: e.code, message: ts.flattenDiagnosticMessageText(e.messageText, '\n') })) }));
      const admitted = !unrelated.length && attempts.find(a => !a.typingErrors.length), { candidates, ...base } = row;
      if (!admitted) { refusals.push({ package: name, export: row.export, reason: unrelated.length ? 'Published dependency typing errors' : 'No type-valid callback witness', attempts }); continue; }
      const parsed = ts.createSourceFile('probe.ts', `candidate(${admitted.arguments.join(', ')});`, ts.ScriptTarget.Latest, true), args = callbackArguments(parsed.statements[0].expression.arguments);
      const finalChecker = checked.getTypeChecker(), call = checkedSource.statements[admitted.id + 1].body.statements[0].expression, returnType = finalChecker.getTypeAtLocation(call);
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
const report = { authority: false, certification: false, basis: 'exact imported value members, published types and standard binary witnesses',
  inputRun: { path: runPath, sha256: hash(readFileSync(runPath)) }, catalogPath, catalogSha256: hash(readFileSync(catalogPath)),
  selectionProducer: new URL('./surface-selection.mjs', import.meta.url).pathname, rows, refusals, packages,
  summary: { roots: packages.length, members: packages.reduce((n,p) => n+p.members,0), callableMembers: packages.reduce((n,p) => n+p.callableMembers,0),
    candidates: packages.reduce((n,p) => n+p.candidates,0), admittedExports: rows.length, admittedPackages: new Set(rows.map(r => r.package)).size,
    rootsRefused: packages.filter(p => p.error).length, sourceAssumptions: rows.filter(r => r.callbackSource.assumptions.length).length } };
writeFileSync(output, JSON.stringify(report, null, 2) + '\n'); console.log(JSON.stringify(report.summary, null, 2));
