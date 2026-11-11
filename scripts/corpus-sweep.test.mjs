// scripts/corpus-sweep.mjs, run as the script it is against a one-project corpus and a fake checker: the sweep is a top-level script (nothing can
// import its intake), so the test runs it. ADR 0950: a sweep report is a baseline, and a what-if report is a prediction, so the first one stops
// the sweep and nothing is written.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "vitest";

import { TAINTED, cleanReport, fakeChecker } from "./lib/assumed-fixtures.mjs";

const script = new URL("./corpus-sweep.mjs", import.meta.url).pathname;
const commit = "35c4cc6b7c932f5dee5815b37d3601e7dff90b58";

/** A corpus of one project, a producer and the checker `report` is printed by, in a throwaway directory; the sweep over it. */
function sweep(root, report) {
  mkdirSync(join(root, "apps", "one"), { recursive: true });
  writeFileSync(join(root, "apps", "one", "tsconfig.json"), "{}\n");
  writeFileSync(join(root, "manifest.json"), JSON.stringify({
    configuration: { runtimeTarget: "browser", runtimeBuild: "development", rendering: "csr" },
    apps: { one: { repository: "https://example.test/one", commit } },
    projects: [{ app: "one", root: "one", project: "tsconfig.json" }],
  }));
  writeFileSync(join(root, "typefacts"), "producer\n");
  const out = join(root, "report.json");
  const run = spawnSync(process.execPath, [
    script, "--manifest", join(root, "manifest.json"), "--apps-root", join(root, "apps"), "--checker", fakeChecker(root, report),
    "--typefacts", join(root, "typefacts"), "--jobs", "1", "--out", out,
  ], { encoding: "utf8" });
  return { run, out };
}

test("a what-if report stops the sweep with the refusal and writes no report", () => {
  for (const [name, make] of Object.entries(TAINTED)) {
    const root = mkdtempSync(join(tmpdir(), "corpus-sweep-what-if-"));
    try {
      const { run, out } = sweep(root, make());
      assert.notEqual(run.status, 0, name);
      assert.match(run.stderr, /scripts\/corpus-sweep\.mjs refuses this checker report: it is tainted by a what-if assumption/, name);
      assert.equal(existsSync(out), false, name);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  }
});

test("a default build's report is swept as before: its findings, status and premises are the project's row", () => {
  const root = mkdtempSync(join(tmpdir(), "corpus-sweep-default-"));
  try {
    const report = { ...cleanReport(), premises: ["standard runtime configuration (ADRs 0266/0268)"] };
    const { run, out } = sweep(root, report);
    assert.equal(run.status, 0, run.stderr);
    const swept = JSON.parse(readFileSync(out, "utf8"));
    assert.deepEqual([swept.projectCount, swept.findingCount, swept.parseErrors, swept.exitErrors], [1, 1, 0, 0]);
    assert.deepEqual(swept.results[0].findings, report.findings);
    assert.deepEqual([swept.results[0].analysisStatus, swept.results[0].premises, swept.results[0].parseError], ["violation", report.premises, null]);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
