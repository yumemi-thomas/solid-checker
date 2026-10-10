// Compare retained unchanged-application outputs by exact diagnostic sites.
// Runtime observations and previously confirmed overclaims are not accuracy rates.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const [beforeArg, afterArg, outputArg] = process.argv.slice(2);
assert(beforeArg && afterArg && outputArg, "Usage: node audit-feedback-precision-fixes.mjs <before-results> <after-results> <fresh-output>");
const output = resolve(outputArg);
assert(!existsSync(output), "Use a fresh output path");
const hash = bytes => `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
const read = path => JSON.parse(readFileSync(path));
const beforePath = resolve(beforeArg), afterPath = resolve(afterArg), before = read(beforePath), after = read(afterPath);
for (const report of [before, after]) {
  assert.equal(report.authority, false); assert.equal(report.certification, false);
  assert(report.finishedAt && report.preservedInputs);
}
// The implementation deliberately changed. Validate the new implementation's
// pins; the old run remains historical evidence with its retained input record.
for (const pin of after.implementation) assert.equal(hash(readFileSync(pin.path)), pin.sha256, pin.path);
const key = finding => JSON.stringify([finding.id, finding.rule, finding.primaryLocation]);
const corrected = [], unchanged = [];
let previousViolations = 0, currentViolations = 0;
for (const id of ["helge-dev", "oscartbeaumont-website"]) {
  const previous = before.results.find(row => row.id === id), current = after.results.find(row => row.id === id);
  assert.deepEqual(current.inputs, previous.inputs, `${id}: application inputs differ`);
  assert.deepEqual(current.runtimePackages, previous.runtimePackages);
  assert.equal(current.commit, previous.commit); assert(current.preservedInputs);
  for (const pin of current.inputs) assert.equal(hash(readFileSync(join(current.root, pin.path))), pin.sha256);
  const oldFeedback = read(join(dirname(beforePath), `${id}.feedback.json`));
  const newFeedback = read(join(dirname(afterPath), `${id}.feedback.json`));
  assert.equal(newFeedback.typingErrorCount, 0);
  previousViolations += oldFeedback.findings.length; currentViolations += newFeedback.findings.length;
  const retained = new Map([...newFeedback.findings, ...newFeedback.gaps].map(finding => [key(finding), finding]));
  for (const finding of oldFeedback.findings) {
    const location = finding.primaryLocation, replacement = retained.get(key(finding));
    if (id === "oscartbeaumont-website" && finding.id === "SC2003" && location.path.endsWith("/src/routes/invoicer/index.tsx")) {
      assert.equal(replacement, undefined, "Nested value write still claims a dropped props write");
      corrected.push({ application: id, code: finding.id, location, change: "removed-unproven-nested-write" });
    } else if (id === "helge-dev" && finding.id === "SC1001" && location.path.endsWith("/src/components/NavBar.tsx")) {
      assert(replacement); assert.equal(replacement.kind, "uncertifiable");
      assert(replacement.message.includes("not proven"));
      corrected.push({ application: id, code: finding.id, location, change: "preserved-callback-invocation-uncertainty" });
    } else {
      assert(replacement); assert.equal(replacement.kind, finding.kind);
      unchanged.push({ application: id, code: finding.id, location });
    }
  }
}
assert.equal(corrected.filter(row => row.change === "removed-unproven-nested-write").length, 14);
assert.equal(corrected.filter(row => row.change === "preserved-callback-invocation-uncertainty").length, 1);
assert.equal(previousViolations, 19); assert.equal(currentViolations, 4); assert.equal(unchanged.length, 4);
const livePath = join(dirname(afterPath), "helge-dev.live.json"), live = read(livePath);
assert.equal(live.execution.configuration.kind, "application-vite-config");
assert.equal(live.execution.configuration.path, join(after.results.find(row => row.id === "helge-dev").root, "vite.config.ts"));
assert.equal(live.execution.failure, null); assert.equal(live.assertions.length, 4);
assert(live.assertions.every(row => row.passed)); assert.deepEqual(live.assertionFailures, []);
assert.deepEqual(live.execution.pageErrors, []); assert.deepEqual(live.execution.blockedRequests, []);
assert.deepEqual(live.execution.consoleDiagnostics, []); assert.equal(live.execution.consoleDiagnosticsDropped, 0);
assert.deepEqual(live.execution.diagnostics, []);
const audit = { authority: false, certification: false, auditedAt: new Date().toISOString(),
  inputs: [beforePath, afterPath, livePath, fileURLToPath(import.meta.url)].map(path => ({ path, sha256: hash(readFileSync(path)) })),
  previousViolations, currentViolations, corrected, unchanged,
  client: { assertionsPassed: 4, configuration: live.execution.configuration,
    observations: live.observations.length, automaticNotes: live.automatic.notes.length,
    readStats: live.execution.stats, unmappedRecords: live.execution.unmapped.length },
  limits: ["The 15 corrected sites were previously reviewed overclaims, not newly detected application defects.",
    "Two remaining loading-context warnings are unvalidated and two findings are preferences.",
    "A successful client scenario and unmapped read records do not establish automatic warning accuracy.",
    "No SSR collector, package causality proof, repair guarantee, or replacement certification is established."] };
writeFileSync(output, JSON.stringify(audit, null, 2) + "\n");
console.log(JSON.stringify({ previousViolations, currentViolations, corrected: corrected.length, client: audit.client }));
