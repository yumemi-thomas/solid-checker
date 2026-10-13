// Re-execute authored consumers without comparisons or supplied assertions.
// Desired values and roles are used only here to score the returned records.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import cases from "./cases.mjs";
const [outputArg, selectionArg] = process.argv.slice(2), output = resolve(outputArg);
assert(!existsSync(output)); mkdirSync(output, { recursive: true });
const configuration = JSON.parse(readFileSync("rust/target/development-broad-config-v1.json", "utf8"));
const bases = ["development-live-cli-v6", "development-live-cli-broad-v1", "development-live-cli-packages-v1"];
const population = bases.flatMap(base => JSON.parse(readFileSync(`rust/target/${base}/results.json`, "utf8")).results.map(row => ({ ...row, base })));
const ids = selectionArg ? JSON.parse(readFileSync(selectionArg, "utf8")).caseIds : population.map(row => row.id);
const hash = bytes => `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
const pins = ["feedback-browser", "feedback-native-hook", "feedback-read-runtime", "feedback-source-hook", "feedback-read-selector", "feedback-assertion-selector", "development-feedback"]
  .map(name => resolve(`packages/cli/scripts/${name}.mjs`)).concat([resolve("packages/cli/bin/solid-checker.mjs"), process.env.SOLID_CHECKER_NATIVE_BIN, process.env.SOLID_TYPEFACTS_BIN])
  .map(path => ({ path, sha256: hash(readFileSync(path)) }));
const results = [];
for (const id of ids) {
  const row = population.find(row => row.id === id), challenge = cases.find(row => row.id === id); assert(row && challenge, id);
  const retained = resolve("rust/target", row.base, id), project = join(retained, "original/tsconfig.json");
  const previous = JSON.parse(readFileSync(join(retained, "scenario.json"), "utf8"));
  const scenario = join(output, id + ".scenario.json");
  writeFileSync(scenario, JSON.stringify({ schemaVersion: 1, steps: [...previous.steps.filter(step => step.action !== "assert-text"),
    { action: "wait-for-selector", selector: "#value" }] }));
  const started = Date.now();
  const run = spawnSync(process.execPath, [resolve("packages/cli/bin/solid-checker.mjs"), "feedback", "run", "--project", project,
    "--scenario", scenario, "--browser", configuration.binaries.chromium, "--tooling", resolve("rust/target/app-import-metric/apps/helge-dev")],
    { encoding: "utf8", maxBuffer: 16 * 1024 * 1024, timeout: 90000 });
  writeFileSync(join(output, id + ".stderr.log"), run.stderr ?? "");
  assert([0, 1].includes(run.status), run.error?.message ?? run.stderr);
  const report = JSON.parse(run.stdout); writeFileSync(join(output, id + ".json"), JSON.stringify(report, null, 2) + "\n");
  assert.equal(report.typingErrorCount, 0); assert.equal(report.comparison, null); assert.deepEqual(report.assertions, []);
  assert.deepEqual(report.execution.pageErrors, []);
  const actual = report.execution.checkpoints.at(-1).text;
  assert.equal(actual, challenge.stages[0].afterUpdate, "Instrumentation changed the measured result");
  results.push({ id, role: challenge.stages[0].role, package: challenge.package, actual, expected: challenge.stages[0].desired,
    automaticWarnings: report.automatic.notes.filter(note => note.severity === "warning").length,
    automaticInformation: report.automatic.notes.filter(note => note.severity === "info").length, open: report.automatic.open.length,
    readObservations: report.observations.filter(note => note.kind === "untracked-read").length, retention: report.execution.stats,
    nativeViolations: report.findings.length, durationMs: Date.now() - started });
  const targets = results.filter(row => row.role === "target"), controls = results.filter(row => row.role === "control");
  const summary = { consumers: results.length, targets: targets.length, targetsWithWarnings: targets.filter(row => row.automaticWarnings).length,
    controls: controls.length, controlsWithWarnings: controls.filter(row => row.automaticWarnings).length };
  writeFileSync(join(output, "results.json"), JSON.stringify({ authority: false, certification: false,
    population: "Prior authored consumers, re-executed without assertions or comparisons; conditional warnings are not errors", pins, summary, results }, null, 2));
  console.log(JSON.stringify(results.at(-1)));
}
for (const pin of pins) assert.equal(hash(readFileSync(pin.path)), pin.sha256);
