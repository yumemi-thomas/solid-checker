import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
const output = resolve(process.argv[2]); assert(!existsSync(output)); mkdirSync(output);
const configuration = JSON.parse(readFileSync("rust/target/development-broad-config-v1.json", "utf8")), results = [];
function run(id, project, scenario) {
  const child = spawnSync(process.execPath, [resolve("packages/cli/bin/solid-checker.mjs"), "feedback", "run", "--project", resolve(project),
    "--scenario", resolve(scenario), "--browser", configuration.binaries.chromium, "--tooling", resolve("rust/target/app-import-metric/apps/helge-dev")],
    { encoding: "utf8", maxBuffer: 16 * 1024 * 1024, timeout: 90000 });
  assert([0, 1].includes(child.status), child.error?.message ?? child.stderr);
  const report = JSON.parse(child.stdout); writeFileSync(join(output, id + ".json"), JSON.stringify(report, null, 2) + "\n"); return report;
}
const plainBase = "rust/target/development-live-cli-boundaries-v1", strictBase = "rust/target/development-live-cli-boundaries-v2";
const plain = run("ordinary-pending-handler", plainBase + "/pending/tsconfig.json", plainBase + "/pending-scenario.json");
assert.equal(plain.typingErrorCount, 0); assert.deepEqual(plain.execution.pageErrors, []);
assert(!plain.findings.some(row => row.id === "SC5001")); assert(plain.gaps.some(row => row.id === "SC5001"));
assert.equal(plain.assertionFailures.length, 0);
results.push({ id: "ordinary-pending-handler", nativePendingViolations: 0, nativePendingGaps: plain.gaps.filter(row => row.id === "SC5001").length, runtimeErrors: 0 });
const strict = run("strict-pending-handler", strictBase + "/pending/tsconfig.json", strictBase + "/pending-scenario.json");
assert.equal(strict.typingErrorCount, 0); assert(strict.observations.some(row => row.kind === "runtime-exception" && row.message.includes("PENDING_ASYNC_UNTRACKED_READ")));
assert.equal(strict.assertionFailures.length, 0);
results.push({ id: "strict-pending-handler", runtimeErrors: strict.observations.filter(row => row.kind === "runtime-exception").length });
const queue = "rust/target/development-live-cli-v6/async-body-return-serial-adopted-child-object-deferred";
const typed = run("published-typing-exclusion", plainBase + "/typed/tsconfig.json", queue + "/scenario.json");
assert(typed.typingErrorCount > 0); assert.deepEqual(typed.automatic.notes, []); assert.deepEqual(typed.assertions, []); assert.deepEqual(typed.observations, []);
results.push({ id: "published-typing-exclusion", typingErrors: typed.typingErrorCount, automaticWarnings: 0 });
const assertion = run("assertion-and-automatic-without-comparison", queue + "/original/tsconfig.json", queue + "/scenario.json");
assert.equal(assertion.typingErrorCount, 0); assert.equal(assertion.assertionFailures.length, 1);
assert.equal(assertion.automatic.notes.length, 1); assert.deepEqual(assertion.guidance.notes, []); assert.equal(assertion.comparison, null);
results.push({ id: "assertion-and-automatic-without-comparison", automaticWarnings: 1, assertionFailures: 1, comparisonGuidance: 0 });
writeFileSync(join(output, "results.json"), JSON.stringify({ authority: false, certification: false, results }, null, 2));
console.log(JSON.stringify(results));
