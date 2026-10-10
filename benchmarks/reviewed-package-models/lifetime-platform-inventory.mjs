// Exact standard-library declaration references in shipped runtime modules.
// Source reach is not execution, consumer coverage or a package behavior model.
import assert from 'node:assert/strict';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { closurePins, hash, nativeRuntimeRoots, packageRoot, read } from './catalog.mjs';
import { runtimeEntry } from './source-extractor.mjs';
import { ts } from './lower.mjs';
const [inputArg, outputArg] = process.argv.slice(2), input = resolve(inputArg), output = resolve(outputArg); assert(!existsSync(output));
const allowedGlobals = new Set(['setTimeout', 'clearTimeout', 'setInterval', 'clearInterval', 'requestAnimationFrame', 'cancelAnimationFrame', 'requestIdleCallback', 'cancelIdleCallback', 'ResizeObserver', 'IntersectionObserver', 'MutationObserver']);
const allowedOwners = new Set(['Window', 'WindowOrWorkerGlobalScope', 'AnimationFrameProvider', 'EventTarget', 'ResizeObserver', 'IntersectionObserver', 'MutationObserver']);
const allowedMembers = new Set(['addEventListener', 'removeEventListener', 'observe', 'unobserve', 'disconnect', ...allowedGlobals]);
export function platformSites(source, checker) {
  const sites = [];
  function visit(node) {
    if (ts.isCallExpression(node) || ts.isNewExpression(node)) {
      const expression = node.expression, symbol = checker.getSymbolAtLocation(ts.isPropertyAccessExpression(expression) ? expression.name : expression);
      if (!symbol?.declarations?.length || !symbol.declarations.every(declaration => declaration.getSourceFile().fileName.endsWith('/lib.dom.d.ts'))) { ts.forEachChild(node, visit); return; }
      for (const declaration of symbol?.declarations ?? []) {
        if (!declaration.getSourceFile().fileName.endsWith('/lib.dom.d.ts')) continue;
        const global = ts.isFunctionDeclaration(declaration) || ts.isVariableDeclaration(declaration), owner = declaration.parent.name?.text;
        const name = declaration.name?.text;
        if (!(global && allowedGlobals.has(name) || ts.isMethodSignature(declaration) && allowedOwners.has(owner) && allowedMembers.has(name))) continue;
        sites.push({ operation: owner ? `${owner}.${name}` : name, kind: ts.isNewExpression(node) ? 'construct' : 'call',
          source: { path: source.fileName, start: node.getStart(source), text: node.getText(source) },
          declaration: { path: declaration.getSourceFile().fileName, start: declaration.getStart(), sha256: hash(declaration.getSourceFile().text) } });
        break;
      }
    }
    ts.forEachChild(node, visit);
  }
  visit(source); return sites;
}
const started = performance.now(), results = [];
for (const row of read(input).results) {
  const result = { package: row.package, version: row.version, modules: [], sites: [], refused: null }; results.push(result);
  try {
    const project = row.retainedArtifacts.projectDir, root = packageRoot(project, row.package), pins = closurePins(root), seen = new Set();
    for (const runtime of nativeRuntimeRoots(project)) assert.equal(read(join(runtime, 'package.json')).version, '2.0.0-rc.9');
    function walk(path) {
      path = resolve(path); assert(path.startsWith(root + '/')); if (seen.has(path)) return; seen.add(path); assert(seen.size < 64);
      const program = ts.createProgram([path], { allowJs: true, noResolve: true, target: ts.ScriptTarget.ESNext });
      const module = { source: program.getSourceFile(path), checker: program.getTypeChecker() }; assert(module.source); assert.deepEqual(module.source.parseDiagnostics, []);
      result.modules.push({ path, sha256: hash(module.source.text) }); result.sites.push(...platformSites(module.source, module.checker));
      for (const statement of module.source.statements) if ((ts.isImportDeclaration(statement) || ts.isExportDeclaration(statement)) && statement.moduleSpecifier?.text?.startsWith('.')) {
        const name = resolve(dirname(path), statement.moduleSpecifier.text), selected = [name, name + '.js', join(name, 'index.js')].find(existsSync);
        if (!selected) throw new Error('Unresolved relative runtime module: ' + name); walk(selected);
      }
    }
    walk(runtimeEntry(join(project, 'App.mjs'), row.package, 'browser')); assert.deepEqual(closurePins(root), pins); result.pins = pins;
  } catch (error) { result.refused = error.message; result.sites = []; }
}
const observed = results.filter(row => !row.refused && row.sites.length);
const summary = { packages: results.length, packagesWithExactPlatformReferences: observed.length, sites: observed.reduce((sum, row) => sum + row.sites.length, 0),
  packagesWithReferences: observed.map(row => ({ package: row.package, operations: [...new Set(row.sites.map(site => site.operation))] })),
  refused: results.filter(row => row.refused).map(row => ({ package: row.package, reason: row.refused })) };
writeFileSync(output, JSON.stringify({ authority: false, certification: false, basis: 'source reach only, not runtime or misuse coverage', typescript: ts.version,
  scriptSha256: hash(readFileSync(new URL(import.meta.url))), input: { path: input, sha256: hash(readFileSync(input)) }, results, summary, durationMs: performance.now() - started }, null, 2) + '\n');
console.log(JSON.stringify(summary, null, 2));
