// Freeze the existing detector before authoring the new consumer population.
// This is research evidence, not package certification authority.
import assert from 'node:assert/strict';
import { existsSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { closurePins, hash, packageRoot, read } from './catalog.mjs';
const [mode, outputArg] = process.argv.slice(2), output = resolve(outputArg);
export const population = ['map', 'set', 'scheduled', 'event-bus', 'date', 'mouse',
  'controlled-signal', 'pagination', 'history', 'promise'].map(name => '@solid-primitives/' + name)
  .concat(['neverthrow', 'zod']);
if (mode === 'capture') {
  assert(!existsSync(output));
  const retained = [...read('rust/target/primitives-checkpoint/run-browser.json').results,
    ...read('rust/target/cross-package-roots/run.json').results];
  const files = readdirSync(new URL('.', import.meta.url)).filter(name => /\.(mjs|json)$/.test(name))
    .map(name => new URL(name, import.meta.url).pathname);
  files.push(resolve('rust/target/debug/solid-checker-rust'), resolve('bin/solid-typefacts'),
    resolve('bin/solid-typefacts.buildinfo'), resolve('scripts/tsc-oracle.mjs'),
    resolve('packages/cli/lib/rules-solid-v2.json'));
  const packages = population.map(name => {
    const row = retained.find(row => row.package === name); assert(row, name);
    const project = row.retainedArtifacts.projectDir, root = packageRoot(project, name);
    return { package: name, project, root, pins: closurePins(root) };
  });
  writeFileSync(output, JSON.stringify({ authority: false, certification: false,
    frozenAt: new Date().toISOString(), population,
    files: files.sort().map(path => ({ path, sha256: hash(readFileSync(path)) })), packages,
    scope: 'Packages held out of the combined 45-consumer matrix; some were sampled in earlier source and callback surveys.',
    protocol: 'Author consumers after this freeze. Keep detector bytes unchanged. Retain misses, unrelated warnings, noisy controls, typing exclusions and harness failures. Freeze the complete consumer population before execution.' }, null, 2) + '\n');
} else {
  assert.equal(mode, 'verify'); const frozen = read(output);
  for (const file of frozen.files) assert.equal(hash(readFileSync(file.path)), file.sha256, file.path);
  for (const pkg of frozen.packages) assert.deepEqual(closurePins(pkg.root), pkg.pins, pkg.package);
  console.log(JSON.stringify({ unchangedFiles: frozen.files.length, unchangedPackages: frozen.packages.length }));
}
