import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { test } from "vitest";

const REPORT = resolve(import.meta.dirname, "../../benchmarks/ecosystem/report.json");
const FULL_CORPUS_ROWS = 418;
// 1200 s (2026-09-13, second re-pin of the day). The 150 s ceiling belonged to
// a corpus that censused three domains; admitting `callbacks` (phase21
// 2026-09-12-callbacks-census-scoping.md § 9) moved the pin to 854 s and the
// budget to 1000 s. ADR 0099 then scheduled a `typeof` veto for every value
// export -- about three thousand more worker launches -- and the same run
// went to 1,205 s; two savings in the ADR 0036 loop (one pass withholds every
// incomplete gate of a batch; the case-set batch synthesizes before the
// per-plan loop) and a cores-bounded certification pool brought the pinned
// run to 981 s with identical certification (docs/precision-backlog.md,
// "Two producer-session and gate-batch savings"). The wall is the
// `solid-js@1.9.14` row, 981 s in the pool against 345 s alone, so the next
// lowering is a decision about that row's contention, not about the budget.
// 1200 s keeps the rule the previous number set: under 1.25x the measured
// wall, so a regression of a quarter of the run is caught while ordinary host
// noise (Low Power Mode, Spotlight indexing leftover temp trees) is not.
// Raising it is a decision to accept a slower certifier, and must not happen
// as a side effect of a re-pin.
const WALL_TIME_BUDGET_MS = 1_200_000;

test("the authoritative full corpus remains below the 1200-second wall-time budget", () => {
  const report = JSON.parse(readFileSync(REPORT, "utf8"));

  assert.equal(report.scope?.kind, "full");
  assert.equal(report.results?.length, FULL_CORPUS_ROWS);
  assert.ok(
    report.durationMs < WALL_TIME_BUDGET_MS,
    `authoritative corpus took ${report.durationMs}ms; budget is strictly below ${WALL_TIME_BUDGET_MS}ms`
  );
});
