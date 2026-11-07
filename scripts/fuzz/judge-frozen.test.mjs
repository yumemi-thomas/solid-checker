import { readFileSync } from "node:fs";
import { describe, expect, test } from "vitest";
import { judgePattern } from "./patterns.mjs";
import { judgeTables } from "./judge-tables.mjs";
import { judge } from "./matrix.mjs";

// Frozen false cleans the judge must flag (endstage-report.md 1d, tier 1). See judge-frozen.json's `about`.
const frozen = JSON.parse(readFileSync(new URL("./judge-frozen.json", import.meta.url), "utf8"));

describe("judge-frozen", () => {
  test("the judge's tables are the frozen ones", () => {
    expect(JSON.parse(JSON.stringify(judgeTables()))).toEqual(frozen.tables);
  });
  test("rows are present", () => {
    expect(frozen.rows.length).toBeGreaterThan(100);
  });
  // ADR 0858: a run that failed. A coded uncaught throw is scored, any other is harness, a site a halted page never reached is unexecuted.
  test.each((frozen.siteRows ?? []).map((row) => [row.id, row]))("%s keeps its site verdict", (_id, row) => {
    const verdict = judge(row.input);
    expect({ outcome: verdict.outcome, ...(row.expected.secondary ? { secondary: verdict.detail?.secondary } : {}) }).toEqual(row.expected);
  });
  test("site rows are present", () => {
    expect((frozen.siteRows ?? []).map((row) => row.expected.outcome).sort()).toEqual(["harness", "not-fired", "unexecuted"]);
  });
  test.each(frozen.rows.map((row) => [row.id, row]))("%s stays a certified false clean", (_id, row) => {
    const verdict = judgePattern({ codes: row.codes }, row.findings);
    expect({ outcome: verdict.outcome, certified: verdict.certified }).toEqual(row.expected);
  });
});
