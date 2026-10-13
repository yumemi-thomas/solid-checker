import assert from "node:assert/strict";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { performance } from "node:perf_hooks";
import { closurePins, hash, nativeRuntimeRoots, packageRoot, read } from "./catalog.mjs";
import { SourceExtractor, runtimeEntry } from "./source-extractor.mjs";
const [runPath, outputPath, filter = ""] = process.argv.slice(2);
assert(runPath && outputPath, "Usage: node extract-catalog.mjs <retained-run.json> <fresh-output.json> [package,...]");
const output = resolve(outputPath); assert(!existsSync(output), "Output already exists");
const run = read(resolve(runPath)), started = performance.now();
const report = { format: "solid-checker-reviewed-model-experiment", version: 1, authority: false, runtime: "2.0.0-rc.9",
  basis: "source-extracted-assumption", extractorSha256: hash(readFileSync(new URL("./source-extractor.mjs", import.meta.url))),
  generatedAt: new Date().toISOString(), models: [], packages: [], summary: null };
for (const row of run.results.filter(item => !filter || filter.split(",").includes(item.package))) {
  const item = { package: row.package, version: row.version, observations: [], error: null }; report.packages.push(item);
  try {
    const root = packageRoot(row.retainedArtifacts.projectDir, row.package), pins = closurePins(root);
    assert.equal(read(join(root, "package.json")).version, row.version, "Retained package version changed");
    for (const pin of pins.filter(item => ["solid-js", "@solidjs/signals", "@solidjs/web"].includes(item.package)))
      assert.equal(pin.version, "2.0.0-rc.9", `Nested runtime mismatch: ${pin.package}`);
    for (const runtime of nativeRuntimeRoots(row.retainedArtifacts.projectDir)) {
      const installed = read(join(runtime, 'package.json'));
      assert.equal(installed.version, "2.0.0-rc.9", `Runtime mismatch: ${installed.name}`);
    }
    const exports = {}, references = new Map();
    for (const host of ["browser", "node"]) {
      const engine = new SourceExtractor(host), entry = runtimeEntry(join(root, "package.json"), row.package, host);
      for (const name of engine.exportNames(entry)) {
        const observation = { host, ...engine.extract(entry, name) }; item.observations.push(observation);
        if (Object.keys(observation.behavior).length) {
          exports[name] ??= {}; exports[name][host] = observation.behavior;
        }
      }
      for (const module of engine.modules.values()) references.set(module.path, { path: module.path, sha256: hash(module.source.text) });
    }
    assert.deepEqual(closurePins(root), pins, "Package inputs changed during extraction");
    if (Object.keys(exports).length) report.models.push({ package: row.package, version: row.version,
      basis: "source-extracted-assumption", pins, exports, sourceReferences: [...references.values()] });
    console.log(`${row.package}: ${Object.keys(exports).length} exports with positive premises`);
  } catch (error) { item.error = error.message; console.log(`${row.package}: refused: ${error.message}`); }
  writeFileSync(output, JSON.stringify(report, null, 2) + "\n");
}
const observations = report.packages.flatMap(item => item.observations);
report.summary = { packages: report.packages.length, refusedPackages: report.packages.filter(item => item.error).length,
  packagesWithPremises: report.models.length, exportHostObservations: observations.length,
  exportHostPremises: observations.filter(item => Object.keys(item.behavior).length).length,
  returnPremises: observations.filter(item => item.behavior.returns).length,
  ownerPremises: observations.filter(item => item.behavior.owner).length,
  durationMs: performance.now() - started };
writeFileSync(output, JSON.stringify(report, null, 2) + "\n"); console.log(JSON.stringify(report.summary, null, 2));
