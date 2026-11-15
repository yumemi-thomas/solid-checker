import { describe, expect, test } from "vitest";
import { overwriteProblem } from "./freeze-guard.mjs";

describe("freeze-judge refuses to overwrite the frozen file (ADR 0995)", () => {
  test("a file that holds rows or changes is never overwritten", () => {
    // The laundering: `node freeze-judge.mjs > judge-frozen.json` after a judge change rewrites every row to the judge's new answer.
    expect(overwriteProblem(JSON.stringify({ rows: [{ id: "a" }], changes: [] }))).toMatch(/already holds rows/);
    expect(overwriteProblem(JSON.stringify({ rows: [], siteRows: [{ id: "a" }] }))).toMatch(/siteRows/);
    expect(overwriteProblem(JSON.stringify({ rows: [], changes: [{ id: "a" }] }))).toMatch(/changes/);
    expect(overwriteProblem("not json")).toMatch(/not JSON/);
  });
  test("an absent or an empty target is written", () => {
    expect(overwriteProblem(null)).toBeNull();
    expect(overwriteProblem(JSON.stringify({ tables: {}, rows: [], changes: [] }))).toBeNull();
  });
});
