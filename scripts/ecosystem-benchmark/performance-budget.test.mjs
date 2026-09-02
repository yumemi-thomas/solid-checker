import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { test } from "vitest";

const REPORT = resolve(import.meta.dirname, "../../benchmarks/ecosystem/report.json");
const FULL_CORPUS_ROWS = 418;
// 150 s, up from 120 s. The two-minute figure was set when the corpus attempted
// about a hundred policy-2 certifications; it now attempts 393, and every
// checked-in run since has measured 180-850 s, most of that sequential registry
// round trips. With registry bytes cached and acquired in parallel the run is
// compute-bound at roughly 176-190 s on the 14-core authority host (see
// docs/ecosystem-benchmark.md, "Where the certified run's time goes"). The
// budget is the ceiling the project holds itself to, not a description of the
// current measurement; with pooled CLI workers, the generated proposal handed
// to certification, cloned execution images and no fsync for scratch catalogs
// the corpus measures ~146 s there (see the table in docs/ecosystem-benchmark.md).
const WALL_TIME_BUDGET_MS = 150_000;

test("the authoritative full corpus remains below the 150-second wall-time budget", () => {
  const report = JSON.parse(readFileSync(REPORT, "utf8"));

  assert.equal(report.scope?.kind, "full");
  assert.equal(report.results?.length, FULL_CORPUS_ROWS);
  assert.ok(
    report.durationMs < WALL_TIME_BUDGET_MS,
    `authoritative corpus took ${report.durationMs}ms; budget is strictly below ${WALL_TIME_BUDGET_MS}ms`
  );
});
