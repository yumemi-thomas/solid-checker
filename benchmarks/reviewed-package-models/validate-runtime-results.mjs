// Validate saved, real-execution samples without rerunning unchanged code.
import assert from "node:assert/strict";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { authenticateModel, hash, read } from "./catalog.mjs";
const [outputArgument, ...inputs] = process.argv.slice(2);
assert(outputArgument && inputs.length, "Usage: node validate-runtime-results.mjs <fresh-output.json> <results.json>...");
const output = resolve(outputArgument); assert(!existsSync(output));
const expected = new Map([
  ["timer-literal-unowned", ["NO_OWNER_CLEANUP"]], ["timer-function-unowned", ["NO_OWNER_EFFECT"]], ["timer-owned", []],
  ["pagination-wrapper-eager", ["STRICT_READ_UNTRACKED", "STRICT_READ_UNTRACKED"]],
  ["pagination-wrapper-tracked", ["STRICT_READ_UNTRACKED"]],
  ["memo-tracked-write", ["REACTIVE_WRITE_IN_OWNED_SCOPE"]], ["memo-tracked-read", []],
]);
const samples = new Map();
for (const argument of inputs) {
  const path = resolve(argument), report = read(path); assert.equal(report.authority, false);
  for (const item of report.results) if (expected.has(item.id)) samples.set(item.id, { path, item });
}
const observations = [];
for (const [id, codes] of expected) {
  const sample = samples.get(id); assert(sample, `Missing runtime sample: ${id}`);
  const { item, path } = sample, dir = join(dirname(path), id), executable = join(dir, "App.mjs");
  assert.equal(item.exitCode, 0, id); assert.equal(item.publishedTypingErrors, 0, id);
  assert.equal(hash(readFileSync(executable)), item.executableSha256, "Executable changed");
  authenticateModel({ package: item.package, version: item.version, pins: item.pins }, dir);
  const feedback = JSON.parse(item.stdout); if (item.feedback) assert.deepEqual(item.feedback, feedback);
  assert.deepEqual(feedback.map(item => item.code), codes, id);
  const lines = readFileSync(executable, "utf8").split("\n").length;
  for (const entry of feedback) {
    assert.equal(entry.basis, "runtime-observation"); assert.equal(entry.certification, false);
    assert.equal(entry.location?.path, pathToFileURL(executable).href);
    assert(entry.location.line > 0 && entry.location.line <= lines && entry.location.column > 0);
  }
  observations.push({ id, package: item.package, version: item.version, evidencePath: path,
    evidenceSha256: hash(readFileSync(path)), executableSha256: item.executableSha256, feedback });
}
writeFileSync(output, JSON.stringify({ authority: false, kind: "validated-saved-runtime-samples", verifiedAt: new Date().toISOString(),
  observations, summary: { samples: observations.length, diagnostics: observations.reduce((sum, item) => sum + item.feedback.length, 0),
    quietSamples: observations.filter(item => !item.feedback.length).length } }, null, 2) + "\n");
console.log("Validated seven saved runtime samples, six diagnostic locations and two quiet samples.");
