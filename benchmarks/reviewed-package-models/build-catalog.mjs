// Deliberate authoring command. Re-pinning requires a fresh source review.
import assert from "node:assert/strict";
import { readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { specs } from "./specs.mjs";
import { closurePins, hash, packageRoot, read } from "./catalog.mjs";
const run = read(resolve(process.argv[2]));
const output = resolve(process.argv[3]);
const models = specs.map(spec => {
  const retained = run.results.find(row => row.package === spec.package && row.version === spec.version);
  assert(retained, `Missing retained install: ${spec.package}`);
  const root = packageRoot(retained.retainedArtifacts.projectDir, spec.package);
  return { ...spec, basis: "reviewed-source-assumption", pins: closurePins(root),
    sourceReferences: spec.files.map(path => ({ path, sha256: hash(readFileSync(join(root, path))),
      source: readFileSync(join(root, path), "utf8") })) };
});
writeFileSync(output, JSON.stringify({ format: "solid-checker-reviewed-model-experiment", version: 1,
  authority: false, runtime: "2.0.0-rc.9", models }, null, 2) + "\n");
console.log(`Wrote ${models.length} models to ${output}`);
