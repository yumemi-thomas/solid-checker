import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { test } from "vitest";

const REPORT = resolve(import.meta.dirname, "../../benchmarks/ecosystem/report.json");
const FULL_CORPUS_ROWS = 418;
// 1000 s, up from 150 s (2026-09-13). The 150 s ceiling was set for a corpus
// that censused three domains; admitting `callbacks` (phase21
// 2026-09-12-callbacks-census-scoping.md § 9) added about 2,270 s of
// cumulative witness acquisition that the note accepts as inherent, and the
// three heaviest rows alone now take 615-735 s each (solid-js@1.9.14,
// @solidjs/web@2.0.0-rc.3, @kobalte/utils@0.9.2), which is why the Makefile's
// row timeout is 1200 s. Every full run on the authority host since then has
// measured 717-854 s (the 2026-09-13 pin, with 114 recipe gates, is 854 s), so
// 150 s was a number the test could only fail. 1000 s is the ceiling the
// project now holds itself to: under 1.2x the measured wall,
// so a regression of a third of the run is caught while ordinary host noise
// (Low Power Mode, Spotlight indexing leftover temp trees) is not. Lowering it
// again is a decision to spend on pooled workers and a shared execution image,
// as the earlier comment planned; raising it is a decision to accept a slower
// certifier, and neither should happen as a side effect of a re-pin.
const WALL_TIME_BUDGET_MS = 1_000_000;

test("the authoritative full corpus remains below the 1000-second wall-time budget", () => {
  const report = JSON.parse(readFileSync(REPORT, "utf8"));

  assert.equal(report.scope?.kind, "full");
  assert.equal(report.results?.length, FULL_CORPUS_ROWS);
  assert.ok(
    report.durationMs < WALL_TIME_BUDGET_MS,
    `authoritative corpus took ${report.durationMs}ms; budget is strictly below ${WALL_TIME_BUDGET_MS}ms`
  );
});
