// Count admissible source paths, not packages with complete misuse coverage.
import assert from 'node:assert/strict';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { hash, read } from './catalog.mjs';
import { ClassFootprints } from './class-footprints.mjs';
import { runtimeEntry } from './source-extractor.mjs';
import { ts } from './lower.mjs';
const [retainedArgument, outputArgument] = process.argv.slice(2), output = resolve(outputArgument); assert(!existsSync(output));
const results = [], started = performance.now();
for (const row of read(resolve(retainedArgument)).results) {
  const engine = new ClassFootprints(), project = row.retainedArtifacts.projectDir;
  const result = { package: row.package, classes: [], refused: null }; results.push(result);
  try {
    const entry = runtimeEntry(join(project, 'App.mjs'), row.package, 'browser');
    for (const name of engine.source.exportNames(entry)) {
      const target = engine.exported(entry, name);
      if (target?.node && (ts.isClassDeclaration(target.node) || ts.isClassExpression(target.node))) result.classes.push(engine.extract(project, row.package, name));
    }
  } catch (error) { result.refused = error.message; }
}
const classes = results.flatMap(row => row.classes), admitted = classes.filter(model => !model.refused && Object.keys(model.methods).length + Object.keys(model.getters).length);
const summary = { packages: results.length, classExports: classes.length, classExportsWithImmediateFootprints: admitted.length,
  packagesWithImmediateFootprints: results.filter(row => row.classes.some(model => admitted.includes(model))).map(row => row.package),
  methodObservations: admitted.reduce((n, model) => n + Object.keys(model.methods).length, 0), getterObservations: admitted.reduce((n, model) => n + Object.keys(model.getters).length, 0),
  refused: results.filter(row => row.refused).map(row => ({ package: row.package, reason: row.refused })) };
writeFileSync(output, JSON.stringify({ authority: false, basis: 'bounded immediate class source paths; no package coverage verdict',
  inventorySha256: hash(readFileSync(new URL('./class-footprint-inventory.mjs', import.meta.url))), extractorSha256: hash(readFileSync(new URL('./class-footprints.mjs', import.meta.url))),
  summary, results, durationMs: performance.now() - started, finishedAt: new Date().toISOString() }, null, 2) + '\n');
console.log(JSON.stringify(summary, null, 2));
