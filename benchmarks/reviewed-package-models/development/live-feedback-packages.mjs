// Additional installed-package shapes through the same unmodified CLI detector.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, symlinkSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import cases from "./cases.mjs";
const [outArg, referenceArg] = process.argv.slice(2), output = resolve(outArg), reference = resolve(referenceArg);
assert(!existsSync(output)); mkdirSync(output);
const configuration = JSON.parse(readFileSync("rust/target/development-broad-config-v1.json", "utf8"));
const shared = join(reference, "async-body-return-serial-adopted-child-object-deferred", "original");
const rows = ["namespace-map-stale", "namespace-map-tracked", "reexport-controlled-signal-stale", "reexport-controlled-signal-tracked", "delayed-controlled-signal-stale"];
const results = [];
function application(root, row, source) {
  mkdirSync(join(root, "src"), { recursive: true }); symlinkSync(join(row.install, "node_modules"), join(root, "node_modules"), "dir");
  for (const name of ["index.html", "tsconfig.json"]) writeFileSync(join(root, name), readFileSync(join(shared, name)));
  for (const name of ["setup.ts", "entry.ts"]) writeFileSync(join(root, "src", name), readFileSync(join(shared, "src", name)));
  writeFileSync(join(root, "src/main.tsx"), source); writeFileSync(join(root, "src/consumer.ts"), row.stages[0].helper);
  for (const [name, code] of Object.entries(row.files ?? {})) writeFileSync(join(root, "src", name), code);
  return join(root, "tsconfig.json");
}
for (const id of rows) {
  const row = cases.find(row => row.id === id), stage = row.stages[0], root = join(output, id); mkdirSync(root);
  let alternative = row.source;
  if (id.endsWith("-stale") && !id.startsWith("delayed")) alternative = cases.find(row => row.id === id.replace(/-stale$/, "-tracked")).source;
  if (id.startsWith("delayed")) {
    const needle = "return Promise.resolve().then(()=>invoke(get));";
    assert.equal(alternative.split(needle).length, 2);
    alternative = alternative.replace(needle, "const captured=get();return Promise.resolve().then(()=>invoke(()=>captured));");
  }
  const original = application(join(root, "original"), row, row.source), comparison = application(join(root, "comparison"), row, alternative);
  const scenario = join(root, "scenario.json");
  writeFileSync(scenario, JSON.stringify({ schemaVersion: 1, steps: [
    { action: "wait-for-text", selector: "#value", text: stage.initial },
    { action: "wait-for-selector", selector: 'body[data-ready="true"]' },
    { action: "click", selector: "#update" },
    { action: "wait-for-selector", selector: 'body[data-ready="true"]' },
    { action: "assert-text", id: "updated-value", selector: "#value", text: stage.desired }
  ] }));
  const run = spawnSync(process.execPath, [resolve("packages/cli/bin/solid-checker.mjs"), "feedback", "run", "--project", original,
    "--compare-project", comparison, "--scenario", scenario, "--browser", configuration.binaries.chromium,
    "--tooling", resolve("rust/target/app-import-metric/apps/helge-dev")], { encoding: "utf8", maxBuffer: 16 * 1024 * 1024, timeout: 90000 });
  assert([0, 1].includes(run.status), run.error?.message ?? run.stderr);
  const report = JSON.parse(run.stdout); writeFileSync(join(root, "feedback.json"), JSON.stringify(report, null, 2) + "\n");
  assert.equal(report.typingErrorCount, 0); assert.equal(report.comparison.typingErrors, 0);
  assert.equal(report.assertions[0].actual, stage.afterUpdate);
  assert.equal(report.guidance.notes.length, stage.role === "target" ? 1 : 0);
  assert.equal(report.assertionFailures.length, stage.role === "target" ? 1 : 0);
  assert.deepEqual(report.execution.pageErrors, []); assert.deepEqual(report.comparison.pageErrors, []);
  assert(report.observations.some(row => row.kind === "untracked-read"));
  results.push({ id, package: row.package, version: JSON.parse(readFileSync(join(row.install, "node_modules", row.package, "package.json"))).version,
    role: stage.role, assertionFailures: report.assertionFailures.length, guidance: report.guidance.notes.length,
    actual: report.assertions[0].actual, expected: stage.desired, comparison: report.comparison.assertions[0].actual,
    readObservations: report.observations.filter(row => row.kind === "untracked-read").length });
  writeFileSync(join(output, "results.json"), JSON.stringify({ authority: false, certification: false, results }, null, 2) + "\n");
  console.log(JSON.stringify(results.at(-1)));
}
