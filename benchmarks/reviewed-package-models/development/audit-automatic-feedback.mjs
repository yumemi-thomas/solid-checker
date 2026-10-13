// Independent scoring and identity audit. This does not import the automatic
// selector or consume its benchmark expectations as selection inputs.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import cases from "./cases.mjs";
const [baseArg, lineageArg, boundariesArg, outputArg] = process.argv.slice(2), base = resolve(baseArg);
const hash = bytes => `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
const collection = JSON.parse(readFileSync(join(base, "results.json"), "utf8"));
assert.equal(collection.results.length, 41);
for (const pin of collection.pins) assert.equal(hash(readFileSync(pin.path)), pin.sha256, pin.path);
const rows = [], packages = new Map();
for (const result of collection.results) {
  const challenge = cases.find(row => row.id === result.id), report = JSON.parse(readFileSync(join(base, result.id + ".json"), "utf8"));
  assert(challenge); assert.equal(report.typingErrorCount, 0); assert.equal(report.comparison, null);
  assert.deepEqual(report.assertions, []); assert.deepEqual(report.guidance.notes, []); assert.deepEqual(report.execution.pageErrors, []);
  assert.equal(report.execution.checkpoints.at(-1).text, challenge.stages[0].afterUpdate);
  assert.equal(report.execution.stats.dropped, 0); assert.equal(report.findings.length, 0);
  for (const model of report.analysis.feedbackFacts) assert.equal(hash(readFileSync(model.path)), model.sourceSha256, model.path);
  for (const note of report.automatic.notes) {
    assert.equal(note.authority, false); assert.equal(note.certification, false); assert.equal(note.reactiveIntent, "open");
    if (note.severity === "info") assert.equal(note.reactiveRead, "unproven");
    else { assert.equal(note.severity, "warning"); assert.equal(note.promiseSettlement, "unproven"); }
  }
  const warnings = report.automatic.notes.filter(row => row.severity === "warning").length;
  const information = report.automatic.notes.filter(row => row.severity === "info").length;
  if (challenge.stages[0].role === "control") { assert.equal(warnings, 0); assert.equal(information, 0); }
  const metadata = join(challenge.install, "node_modules", challenge.package, "package.json"), version = JSON.parse(readFileSync(metadata)).version;
  packages.set(`${challenge.package}@${version}`, { name: challenge.package, version, metadataSha256: hash(readFileSync(metadata)) });
  rows.push({ id: result.id, role: challenge.stages[0].role, package: challenge.package, version, actual: result.actual,
    expected: challenge.stages[0].desired, warnings, information });
}
const lineage = JSON.parse(readFileSync(join(resolve(lineageArg), "results.json"), "utf8"));
assert.equal(lineage.results.length, 6);
for (const row of lineage.results) assert.equal(row.automaticWarnings, row.role === "target" ? 1 : 0);
const boundaries = JSON.parse(readFileSync(join(resolve(boundariesArg), "results.json"), "utf8")); assert.equal(boundaries.results.length, 4);
const targets = rows.filter(row => row.role === "target"), controls = rows.filter(row => row.role === "control");
const summary = { consumers: rows.length, targets: targets.length, targetsWithWarnings: targets.filter(row => row.warnings).length,
  targetsWithInformation: targets.filter(row => !row.warnings && row.information).length,
  targetsStillOpen: targets.filter(row => !row.warnings && !row.information).map(row => row.id),
  controls: controls.length, controlsWithGuidance: controls.filter(row => row.warnings || row.information).length,
  freshLineageTargets: 2, freshLineageControls: 4, boundaryChecks: 4 };
const evidence = { schemaVersion: 1, authority: false, certification: false,
  population: "41 prior authored consumers without assertions/comparison code, six fresh lineage cases, four runtime/typing boundary checks; no universal accuracy claim",
  summary, packages: [...packages.values()], implementation: collection.pins, rows, lineage: lineage.results, boundaries: boundaries.results };
writeFileSync(resolve(outputArg), JSON.stringify(evidence, null, 2) + "\n"); console.log(JSON.stringify(summary));
