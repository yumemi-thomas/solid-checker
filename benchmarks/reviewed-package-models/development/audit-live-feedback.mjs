// Reconstruct decisions from scenario assertions and measured values without
// importing the shipped selector. Roles below are scoring inputs only.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
const [outArg, ...runs] = process.argv.slice(2), output = resolve(outArg);
const hash = bytes => `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
const inputs = [], results = [];
function pinnedJson(path) {
  path = resolve(path); const bytes = readFileSync(path); inputs.push({ path, sha256: hash(bytes) }); return JSON.parse(bytes);
}
for (const arg of runs) {
  const directory = resolve(arg), summary = pinnedJson(join(directory, "results.json"));
  for (const row of summary.results) {
    const report = pinnedJson(join(directory, row.id, "feedback.json"));
    const scenario = pinnedJson(report.scenario.path);
    assert.equal(report.scenario.sha256, inputs.at(-1).sha256);
    const assertions = scenario.steps.filter(step => step.action === "assert-text"), expectedNotes = [];
    assert.equal(report.typingErrorCount, 0); assert.equal(report.comparison.typingErrors, 0);
    assert.equal(report.coverage.runtime, "executed-browser"); assert.equal(report.coverage.packageInference, "unavailable");
    assert.equal(report.findings.length, 0); assert.deepEqual(report.execution.pageErrors, []);
    for (const assertion of assertions) {
      const before = report.assertions.find(item => item.id === assertion.id), after = report.comparison.assertions.find(item => item.id === assertion.id);
      assert.equal(before.selector, assertion.selector); assert.equal(after.selector, assertion.selector);
      assert.equal(before.expected, assertion.text); assert.equal(after.expected, assertion.text);
      if (before.actual !== assertion.text && after.actual === assertion.text) expectedNotes.push(assertion.id);
    }
    assert.deepEqual(report.guidance.notes.map(note => note.id), expectedNotes);
    assert.equal(report.assertionFailures.length, report.assertions.filter(item => item.actual !== item.expected).length);
    for (const note of report.guidance.notes) { assert.equal(note.authority, false); assert.equal(note.certification, false); assert.equal(note.repairSafety, "unproved"); }
    for (const observation of report.observations) {
      assert.equal(observation.kind, "untracked-read"); assert.equal(observation.severity, "info");
      const bytes = readFileSync(observation.location.path); assert.equal(hash(bytes), observation.sourceSha256);
      assert(observation.location.startByte >= 0 && observation.location.endByte <= bytes.length);
    }
    const profile = report.coverage.nativeReadCollection.nativeReader; assert.equal(hash(readFileSync(profile.path)), profile.sha256);
    results.push({ id: row.id, role: row.role, assertionFailures: report.assertionFailures.length,
      guidance: expectedNotes.length, readObservations: report.observations.length, nativeViolations: report.findings.length,
      typingErrors: report.typingErrorCount, before: report.assertions.map(item => item.actual),
      after: report.comparison.assertions.map(item => item.actual), expected: assertions.map(item => item.text) });
  }
}
const targets = results.filter(row => row.role === "target"), controls = results.filter(row => row.role === "control");
const summary = { cases: results.length, targets: targets.length, targetFailuresAndGuidance: targets.filter(row => row.assertionFailures === 1 && row.guidance === 1).length,
  controls: controls.length, quietControls: controls.filter(row => row.assertionFailures === 0 && row.guidance === 0 && row.nativeViolations === 0).length };
const source = new URL(import.meta.url).pathname;
writeFileSync(output, JSON.stringify({ authority: false, certification: false,
  scope: "Executed authored primary-value assertions and supplied comparisons; read observations remain informational; no held-out or universal accuracy claim",
  validator: { path: source, sha256: hash(readFileSync(source)) }, inputs, summary, results }, null, 2) + "\n");
console.log(JSON.stringify(summary));
