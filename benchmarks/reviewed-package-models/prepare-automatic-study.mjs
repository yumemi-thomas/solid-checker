import assert from "node:assert/strict";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import cases from "./automatic-feedback-cases.mjs";
import { hash } from "./catalog.mjs";
assert(!process.env.REVIEWED_MODEL_AUTOMATIC_PROFILE, "Freeze all profiles together");
const output = resolve(process.argv[2]); assert(!existsSync(output));
const names = ['automatic-feedback-cases.mjs', 'extended-runtime-feedback.mjs', 'origin-trace.mjs', 'origin-trace-runtime.mjs', 'guard-trace.mjs', 'guard-trace-runtime.mjs'];
writeFileSync(output, JSON.stringify({ authority: false, createdAt: new Date().toISOString(), independentLabels: false,
  inputs: names.map(name => ({ name, sha256: hash(readFileSync(new URL(name, import.meta.url))) })),
  cases: cases.map(entry => ({ id: entry.id, provenance: entry.provenance, sourceSha256: hash(entry.source) })),
}, null, 2) + '\n');
console.log(`Recorded ${cases.length} pre-execution labels`);
