import assert from "node:assert/strict";
import { existsSync, writeFileSync } from "node:fs";
import { performance } from "node:perf_hooks";
import { authenticateModel, read } from "./catalog.mjs";
const output = process.argv[2]; assert(output && !existsSync(output));
const catalog = read("rust/target/reviewed-models-further-catalog-final.json"), retained = read("rust/target/primitives-checkpoint/run-browser.json");
const samples = [];
for (const count of [1, 10, 30, catalog.models.length]) for (let repeat = 0; repeat < 3; repeat++) {
  const start = performance.now();
  for (const model of catalog.models.slice(0, count)) {
    const row = retained.results.find(row => row.package === model.package && row.version === model.version); assert(row);
    authenticateModel(model, row.retainedArtifacts.projectDir);
  }
  samples.push({ packageCount: count, repeat, authenticationMs: performance.now() - start });
}
const report = { authority: false, measurement: "sequential whole-closure reauthentication only", repeatedDependencyHashing: true,
  excludes: ["source extraction", "consumer typing", "native analysis", "combined real application"], samples,
  summary: [...new Set(samples.map(row => row.packageCount))].map(count => { const values = samples.filter(row => row.packageCount === count).map(row => row.authenticationMs).toSorted((a, b) => a - b); return { packageCount: count, medianMs: values[1], maxMs: values.at(-1) }; }) };
writeFileSync(output, JSON.stringify(report, null, 2) + "\n"); console.log(JSON.stringify(report.summary));
