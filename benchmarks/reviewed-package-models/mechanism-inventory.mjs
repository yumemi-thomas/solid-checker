// Inventory exact core calls in the browser root entry's local module graph.
// A hook site is potential observability, never a package-coverage verdict.
import assert from "node:assert/strict";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { hash, nativeRuntimeRoots, packageRoot, read } from "./catalog.mjs";
import { runtimeEntry } from "./source-extractor.mjs";
import { ts } from "./lower.mjs";
const [retainedArgument, outputArgument] = process.argv.slice(2), output = resolve(outputArgument); assert(!existsSync(output));
const groups = {
  reactive: ['createSignal', 'createMemo', 'createStore', 'createProjection', 'createOptimistic'],
  owned: ['createEffect', 'createTrackedEffect', 'onCleanup', 'onSettled', 'createRoot', 'runWithOwner'],
  guarded: ['getOwner', 'getObserver'],
};
const results = [];
for (const row of read(resolve(retainedArgument)).results) {
  const result = { package: row.package, version: row.version, modules: [], sites: [], externalImports: [], gaps: [], refused: null }; results.push(result);
  try {
    const project = row.retainedArtifacts.projectDir, root = packageRoot(project, row.package);
    result.runtime = nativeRuntimeRoots(project).map(path => ({ name: read(join(path, 'package.json')).name, version: read(join(path, 'package.json')).version }));
    const pending = [runtimeEntry(join(project, 'App.mjs'), row.package, 'browser')], visited = new Set();
    while (pending.length) {
      const path = pending.pop(); if (visited.has(path)) continue; visited.add(path);
      if (visited.size > 256) { result.gaps.push('local module budget'); break; }
      assert(path.startsWith(root + '/')); const text = readFileSync(path, 'utf8');
      const program = ts.createProgram([path], { allowJs: true, noResolve: true, noLib: true, target: ts.ScriptTarget.ESNext });
      const source = program.getSourceFile(path), checker = program.getTypeChecker(), bindings = new Map();
      if (!source || source.parseDiagnostics.length) { result.gaps.push('unparsed module: ' + path); continue; }
      result.modules.push({ path, sha256: hash(text) });
      for (const node of source.statements) if ((ts.isImportDeclaration(node) || ts.isExportDeclaration(node)) && node.moduleSpecifier && ts.isStringLiteral(node.moduleSpecifier)) {
        const specifier = node.moduleSpecifier.text;
        if (specifier.startsWith('.')) { try { pending.push(runtimeEntry(path, specifier, 'browser')); } catch (error) { result.gaps.push(error.message); } }
        else result.externalImports.push(specifier);
        if (ts.isImportDeclaration(node) && ['solid-js', '@solidjs/signals'].includes(specifier)) {
          const names = node.importClause?.namedBindings;
          if (names && ts.isNamedImports(names)) for (const name of names.elements) bindings.set(checker.getSymbolAtLocation(name.name), (name.propertyName ?? name.name).text);
          else if (names) result.gaps.push('namespace core binding: ' + path);
        }
      }
      function visit(node) {
        if (ts.isCallExpression(node) && ts.isIdentifier(node.expression)) {
          const api = bindings.get(checker.getSymbolAtLocation(node.expression));
          if (api) { const position = source.getLineAndCharacterOfPosition(node.getStart(source)); result.sites.push({ api, path, line: position.line + 1, start: node.getStart(source) }); }
        }
        ts.forEachChild(node, visit);
      }
      visit(source);
    }
    result.externalImports = [...new Set(result.externalImports)].sort();
    result.groups = Object.fromEntries(Object.entries(groups).map(([name, apis]) => [name, result.sites.filter(site => apis.includes(site.api)).length]));
  } catch (error) { result.refused = error.message; }
}
const summary = { packages: results.length, resolvedRootGraphs: results.filter(row => !row.refused).length,
  anyExactCoreCalls: results.filter(row => row.sites.length).length,
  groups: Object.fromEntries(Object.keys(groups).map(group => [group, results.filter(row => row.groups?.[group]).length])),
  onSettledPackages: results.filter(row => row.sites.some(site => site.api === 'onSettled')).map(row => row.package),
  zeroDirectCoreCallPackages: results.filter(row => !row.refused && !row.sites.length).map(row => row.package),
  gaps: results.filter(row => row.gaps.length).map(row => ({ package: row.package, gaps: row.gaps })),
  refused: results.filter(row => row.refused).map(row => ({ package: row.package, reason: row.refused })) };
writeFileSync(output, JSON.stringify({ authority: false, basis: 'resolved local module inventory, not exercised API coverage',
  inventorySha256: hash(readFileSync(new URL('./mechanism-inventory.mjs', import.meta.url))), summary, results, finishedAt: new Date().toISOString() }, null, 2) + '\n');
console.log(JSON.stringify(summary, null, 2));
