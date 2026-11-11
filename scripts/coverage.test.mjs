// scripts/coverage.mjs, run as the script it is over the real fixture projects with a fake checker: the gate is a top-level script (nothing can
// import its intake), so the test runs it. ADR 0950: a snapshot records a decision about the shipped checker, and a what-if report is a
// prediction, so the first one stops the gate before anything is compared or written.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "vitest";

import { fakeChecker, flippedReport } from "./lib/assumed-fixtures.mjs";

const script = new URL("./coverage.mjs", import.meta.url).pathname;
const snapshots = new URL("../fixtures/findings-snapshots", import.meta.url).pathname;

/** The gate in compare mode (never `--update`), its cache off, over the fixtures, with a checker that prints `report`. */
function gate(report) {
  const root = mkdtempSync(join(tmpdir(), "coverage-what-if-"));
  try {
    writeFileSync(join(root, "typefacts"), "producer\n");
    const before = readdirSync(snapshots);
    const { SOLID_COVERAGE_REPORT: _report, ...inherited } = process.env;
    const run = spawnSync(process.execPath, [script], {
      encoding: "utf8",
      maxBuffer: 1 << 28,
      env: { ...inherited, SOLID_CHECKER_BIN: fakeChecker(root, report), SOLID_TYPEFACTS_BIN: join(root, "typefacts"), SOLID_CHECKER_GATE_CACHE: "0" },
    });
    assert.deepEqual(readdirSync(snapshots), before, "compare mode neither records nor prunes a snapshot");
    return run;
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("a what-if report stops the gate with the refusal: nothing is compared", () => {
  const run = gate(flippedReport());
  assert.notEqual(run.status, 0);
  assert.match(run.stderr, /scripts\/coverage\.mjs refuses this checker report: it is tainted by a what-if assumption/);
  assert.doesNotMatch(run.stderr + run.stdout, /findings moved|compared \d+ fixture projects/);
});

test("a default build's report passes the intake: every fixture project is compared against its snapshot", () => {
  // The fake checker finds nothing anywhere, so the snapshots that pinned findings differ and the gate fails: that failure is the comparison
  // running. A refusal would have stopped it before the first project.
  const run = gate({ status: "certified", findings: [] });
  assert.equal(run.status, 1);
  assert.doesNotMatch(run.stderr, /refuses this checker report/);
  assert.match(run.stderr, /findings moved: /);
  assert.match(run.stdout, /compared \d+ fixture projects, 0 findings/);
});
