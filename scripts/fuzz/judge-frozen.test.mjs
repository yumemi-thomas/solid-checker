import { readFileSync } from "node:fs";
import { describe, expect, test } from "vitest";
import { judgePattern } from "./patterns.mjs";
import { judgeTables } from "./judge-tables.mjs";
import { evaluatedDead, judgeLegal } from "./legal.mjs";
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
  // ADR 0815, R21: a run-claiming violation at a site the text says never runs is wrong; at one only an unknown receiver skipped, unexecuted.
  test.each(frozen.legalRows.map((row) => [row.id, row]))("%s keeps its legal verdict", (_id, row) => {
    const evaluated = evaluatedDead(null, { ledger: [], files: { "App.tsx": row.text } }, row.input.site);
    expect(judgeLegal({ ...row.input, evaluated }).outcome).toBe(row.expected.outcome);
  });
  test("site rows are present", () => {
    // ADR 0887 added five: a halted run's stacked claim (proved), its unproved twin, nothing of the rule, the rule's code elsewhere, a line never
    // run; then four: a proved repeat, a repeat of another source, an unproved repeat, an unjudged claim; then a repeat whose line never ran.
    expect((frozen.siteRows ?? []).map((row) => row.expected.outcome).sort()).toEqual(["agree", "false-clean", "false-clean", "false-positive", "false-positive", "false-positive", "false-positive", "false-positive", "harness", "not-fired", "not-fired", "recall-gap", "recall-gap", "stacked-claim", "stacked-claim", "unexecuted", "unjudged-claim"]);
  });
  test.each(frozen.rows.map((row) => [row.id, row]))("%s keeps its frozen verdict", (_id, row) => {
    const verdict = judgePattern({ codes: row.codes, stacks: row.stacks, positions: row.stacks !== undefined }, row.findings);
    expect({ outcome: verdict.outcome, certified: verdict.certified }).toEqual(row.expected);
  });
});
