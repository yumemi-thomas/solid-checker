import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { test } from "vitest";

const REPORT = resolve(import.meta.dirname, "../../benchmarks/ecosystem/report.json");
const FULL_CORPUS_ROWS = 418;
const WALL_TIME_BUDGET_MS = 120_000;

test("the authoritative full corpus remains below the two-minute wall-time budget", () => {
  const report = JSON.parse(readFileSync(REPORT, "utf8"));

  assert.equal(report.scope?.kind, "full");
  assert.equal(report.results?.length, FULL_CORPUS_ROWS);
  assert.ok(
    report.durationMs < WALL_TIME_BUDGET_MS,
    `authoritative corpus took ${report.durationMs}ms; budget is strictly below ${WALL_TIME_BUDGET_MS}ms`
  );
});
