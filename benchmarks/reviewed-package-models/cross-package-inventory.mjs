// A declared population from multiple ecosystems. These names choose the
// sample; argument generation and execution do not use package-specific rules.
import assert from 'node:assert/strict';
import { existsSync, mkdirSync, readFileSync, symlinkSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { closurePins, hash, nativeRuntimeRoots, packageRoot, read } from './catalog.mjs';

const population = ['@floating-ui/dom', '@sparkstone/solid-validation', '@solidjs/meta', '@solidjs/router',
  '@tanstack/query-core', '@tanstack/virtual-core', 'clsx', 'eventemitter3', 'fflate', 'lodash',
  'lodash-es', 'mitt', 'nanoid', 'neverthrow', 'p-limit', 'rxjs', 'seroval', 'solid-relay',
  'tailwind-merge', 'ts-pattern', 'valibot', 'zod'];
const [appsArg, runtimeArg, outputArg] = process.argv.slice(2);
const apps = resolve(appsArg), runtimeProject = resolve(runtimeArg), output = resolve(outputArg);
assert(!existsSync(output)); mkdirSync(output, { recursive: true });
const listing = spawnSync('rg', ['--files', apps, '-g', 'package.json'], { encoding: 'utf8' });
assert.equal(listing.status, 0);
const manifests = listing.stdout.trim().split('\n').sort().map(path => ({ path, manifest: read(path) }));
const runtime = nativeRuntimeRoots(runtimeProject);
for (const root of runtime) assert.equal(read(join(root, 'package.json')).version, '2.0.0-rc.9');
const rows = [], results = [];
for (const name of population) {
  const item = { package: name, candidates: [], selected: null }; rows.push(item);
  const seen = new Set();
  for (const { path, manifest } of manifests) {
    if (!Object.hasOwn({ ...manifest.dependencies, ...manifest.devDependencies }, name)) continue;
    const app = dirname(path);
    try {
      const root = packageRoot(app, name); if (seen.has(root)) continue; seen.add(root);
      const installed = read(join(root, 'package.json')), pins = closurePins(root);
      for (const pin of pins.filter(p => ['solid-js', '@solidjs/signals', '@solidjs/web'].includes(p.package)))
        assert.equal(pin.version, '2.0.0-rc.9', 'Package closure uses another Solid runtime');
      if (!item.selected) {
        const project = join(output, hash(name).slice(7, 19)), modules = join(project, 'node_modules');
        mkdirSync(join(project, 'src'), { recursive: true }); mkdirSync(modules);
        const links = [[name, root], ...runtime.map(root => [read(join(root, 'package.json')).name, root])];
        for (const [packageName, target] of links) {
          const destination = join(modules, packageName); mkdirSync(dirname(destination), { recursive: true });
          symlinkSync(target, destination, 'dir');
        }
        const types = join(app, 'node_modules/@types');
        if (existsSync(types)) symlinkSync(types, join(modules, '@types'), 'dir');
        writeFileSync(join(project, 'package.json'), JSON.stringify({ private: true, type: 'module' }) + '\n');
        item.selected = { version: installed.version, root, sourceApp: app, project, pins,
          sourceManifest: { path, sha256: hash(readFileSync(path)) }, typeNamespace: existsSync(types) ? types : null };
        results.push({ package: name, retainedArtifacts: { projectDir: project } });
      }
      item.candidates.push({ root, version: installed.version, admitted: true });
    } catch (error) { item.candidates.push({ app, admitted: false, reason: error.message }); }
  }
}
const summary = { requestedPackages: population.length, availablePackages: rows.filter(r => r.candidates.length).length,
  admittedPackages: results.length, packagesRefused: rows.filter(r => r.candidates.length && !r.selected).length,
  packagesAbsent: rows.filter(r => !r.candidates.length).length };
writeFileSync(join(output, 'run.json'), JSON.stringify({ authority: false, certification: false,
  basis: 'declared cross-ecosystem population using exact cached package bytes and rc.9 consumer runtime',
  population, runtime: runtime.map(root => ({ root, version: read(join(root, 'package.json')).version })),
  rows, results, summary }, null, 2) + '\n');
console.log(JSON.stringify(summary, null, 2));
