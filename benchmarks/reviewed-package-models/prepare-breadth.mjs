// Record the authored expectations and freeze the existing static prototype
// before running a fresh breadth study. This does not grant proof authority.
import assert from "node:assert/strict";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import cases from "./breadth-cases.mjs";
import { hash } from "./catalog.mjs";

const output = resolve(process.argv[2]);
assert(!existsSync(output), "Study output must be fresh");
const frozen = ["source-extractor.mjs", "lower.mjs", "demand-models.mjs"].map(name => ({
  name, digest: hash(readFileSync(new URL(name, import.meta.url))),
}));
writeFileSync(output, JSON.stringify({
  authority: false,
  createdAt: new Date().toISOString(),
  frozen,
  corpusSha256: hash(readFileSync(new URL("./breadth-cases.mjs", import.meta.url))),
  cases: cases.map(entry => ({
    id: entry.id, package: entry.package, app: entry.app,
    provenance: entry.provenance,
    sourceSha256: entry.source == null ? null : hash(entry.source),
  })),
  independentHumanLabels: false,
}, null, 2) + "\n");
console.log(`Recorded ${cases.length} consumers and ${frozen.length} frozen inputs`);
