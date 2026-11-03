// The fuzzer's own tests (scripts/fuzz/*.test.mjs) as a lane of `make verify` and CI. They were outside both, and a duplicate role-log
// arm id (`callee_roles.named-callback` tagged twice, which roles.test.mjs fails) reached main. This lane has no silent skip:
//
//   - what the tests need is provisioned (`--provision`, which verify's first step and CI run) or the lane fails at once naming the
//     command that provisions it (provision.mjs); the tests that need an install fail when it is absent, never skip;
//   - a test that vitest skipped fails the lane, except the ones that need a `tsrs` binary when none is found (tsrs is a separate
//     repository and not provisionable here): those are listed by name in the output, never counted as passed. With tsrs present
//     (SOLID_CHECKER_TSRS_BIN or a checkout beside the repository, typecheck.mjs resolveTsc) they must run.
//
// The tests run under Node, not Bun: the fuzzer is Node tooling (README), and ssr.mjs needs `node:module`'s stripTypeScriptTypes, which
// Bun lacks (the same suite under `bun vitest` failed ssr.test.mjs once its install existed). The repository's pin is Node 26.
//
//   bun scripts/fuzz/test-lane.mjs [--provision] [<vitest filter>...]
import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { NODE } from "../lib/node-runtime.mjs";
import { requirements, unmet, unmetMessage } from "./provision.mjs";
import { resolveTsc } from "./typecheck.mjs";

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "../..");

/** The test files whose blocks skip without a tsrs binary (`describe.skipIf(!tsrs)`): the one input the lane cannot provision. */
export const TSRS_ONLY = ["checker.test.mjs", "rewrite.test.mjs", "tsc-overlap.test.mjs"];

/**
 * Splits vitest's skipped tests into those allowed to skip (they need tsrs and there is none) and those that fail the lane.
 * @param {{ file: string, name: string }[]} skipped
 */
export function judgeSkips(skipped, { tsrsPresent }) {
  const allowed = skipped.filter((test) => !tsrsPresent && TSRS_ONLY.includes(basename(test.file)));
  return { allowed, failed: skipped.filter((test) => !allowed.includes(test)) };
}

/** Every skipped (or todo) test in vitest's JSON report. */
export function skippedIn(report) {
  return report.testResults.flatMap((file) => file.assertionResults.filter((test) => test.status !== "passed" && test.status !== "failed").map((test) => ({ file: file.name, name: test.fullName })));
}

function main(argv) {
  const provisioning = argv.includes("--provision");
  const filters = argv.filter((arg) => !arg.startsWith("--"));
  if (provisioning) {
    const result = spawnSync(NODE, [join(repo, "scripts/fuzz/provision.mjs")], { stdio: "inherit" });
    if (result.status !== 0) return 1;
  } else {
    const found = unmet(requirements());
    if (found.length) { console.error(`fuzz tests: ${unmetMessage(found)}`); return 1; }
  }
  const tsrs = resolveTsc();
  const tsrsPresent = existsSync(tsrs);
  const scratch = mkdtempSync(join(tmpdir(), "fuzz-test-lane-"));
  try {
    const reportPath = join(scratch, "report.json");
    const run = spawnSync(NODE, [join(repo, "packages/cli/node_modules/vitest/vitest.mjs"), "run", "--config", "packages/cli/vitest.config.mjs", ...(filters.length ? filters : ["scripts/fuzz/"]), "--reporter=default", "--reporter=json", `--outputFile.json=${reportPath}`], { cwd: repo, stdio: "inherit" });
    if (!existsSync(reportPath)) { console.error(`fuzz tests: vitest wrote no report (exit ${run.status ?? run.error})`); return 1; }
    const report = JSON.parse(readFileSync(reportPath, "utf8"));
    const { allowed, failed } = judgeSkips(skippedIn(report), { tsrsPresent });
    console.log(`\nfuzz tests: ${report.numPassedTests} passed, ${report.numFailedTests} failed, ${allowed.length + failed.length} skipped`);
    if (allowed.length) console.log(`  not run, no tsrs binary (${tsrs}; set SOLID_CHECKER_TSRS_BIN): ${[...new Set(allowed.map((test) => basename(test.file)))].join(", ")} (${allowed.length} tests)`);
    for (const test of failed) console.error(`  SKIPPED (a skip is a failure here): ${basename(test.file)}: ${test.name}`);
    return run.status === 0 && !failed.length ? 0 : 1;
  } finally {
    rmSync(scratch, { recursive: true, force: true });
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) process.exit(main(process.argv.slice(2)));
