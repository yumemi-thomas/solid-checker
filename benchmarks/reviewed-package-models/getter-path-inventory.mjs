// Positive getter paths under unresolved arguments. This is not misuse coverage.
import assert from 'node:assert/strict';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { closurePins, hash, nativeRuntimeRoots, packageRoot, read } from './catalog.mjs';
import { GetterPaths } from './getter-paths.mjs';
import { runtimeEntry, unknownValue } from './source-extractor.mjs';
const [retainedArgument, outputArgument] = process.argv.slice(2), output = resolve(outputArgument); assert(!existsSync(output));
const started = performance.now(), results = [];
for (const row of read(resolve(retainedArgument)).results) {
  const result = { package: row.package, version: row.version, exports: [], refused: null }; results.push(result);
  try {
    const project = row.retainedArtifacts.projectDir, root = packageRoot(project, row.package), pins = closurePins(root), engine = new GetterPaths();
    for (const path of nativeRuntimeRoots(project)) assert.equal(read(join(path, 'package.json')).version, '2.0.0-rc.9');
    for (const pin of pins.filter(pin => ['solid-js', '@solidjs/signals', '@solidjs/web'].includes(pin.package))) assert.equal(pin.version, '2.0.0-rc.9');
    const entry = runtimeEntry(join(project, 'App.mjs'), row.package, 'browser');
    for (const name of engine.exportNames(entry)) {
      const module = engine.module(entry), state = { env: new Map(), owned: false, operations: [], unknowns: [], blockers: [], reads: [] }, target = engine.exportValue(module, name, state, 0);
      if (target.kind !== 'function') continue;
      result.exports.push(engine.extractGetters(entry, name, target.node.parameters.map((_, index) => unknownValue('unresolved parameter ' + index))));
    }
    assert.deepEqual(closurePins(root), pins); result.pins = pins;
    result.sources = [...engine.modules.values()].map(module => ({ path: module.path, sha256: hash(module.source.text) }));
  } catch (error) { result.refused = error.message; }
}
const admitted = results.flatMap(row => row.exports.filter(entry => entry.fields.length).map(entry => ({ package: row.package, export: entry.name, fields: entry.fields.map(field => field.path) })));
const summary = { packages: results.length, functionExportsInspected: results.reduce((count, row) => count + row.exports.length, 0),
  exportsWithFiniteGetterPaths: admitted.length, packagesWithFiniteGetterPaths: [...new Set(admitted.map(row => row.package))], admitted,
  refused: results.filter(row => row.refused).map(row => ({ package: row.package, reason: row.refused })) };
writeFileSync(output, JSON.stringify({ authority: false, certification: false, basis: 'finite source getter paths with unresolved arguments; not consumer or defect coverage',
  extractorSha256: hash(readFileSync(new URL('./getter-paths.mjs', import.meta.url))), summary, results, durationMs: performance.now() - started, finishedAt: new Date().toISOString() }, null, 2) + '\n');
console.log(JSON.stringify(summary, null, 2));
