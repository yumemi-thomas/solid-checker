import { describe, expect, it } from "vitest";
import { BLIND, blindFindingStats, compareRuns } from "./neutrality.mjs";

const row = (id, verdict, items = [], extra = {}) => ({ id, verdict, items, ...extra });
const blind = (rule, line) => ({ kind: "inconclusive", reason: BLIND, finding: { rule, line } });
const resolved = (kind, rule, line) => ({ kind, reason: "x", finding: { rule, line } });

describe("blindFindingStats", () => {
  it("counts the findings that used to span no site and now map to one", () => {
    const base = [row("a", "inconclusive", [blind("strict-read-untracked", 12)]), row("b", "inconclusive", [blind("strict-read-untracked", 7), blind("no-destructure", 9)]), row("c", "agree")];
    const instr = [
      row("a", "candidate-false-positive", [resolved("candidate-false-positive", "strict-read-untracked", 12)]),
      row("b", "inconclusive", [resolved("inconclusive", "strict-read-untracked", 7), blind("no-destructure", 9)]),
      row("c", "agree"),
    ];
    expect(blindFindingStats(base, instr)).toEqual({ before: 3, stillBlind: 1, mapped: 2, mappedTo: { "candidate-false-positive": 1, inconclusive: 1 } });
  });
  it("a finding the instrumented run does not mention is not counted as mapped", () => {
    expect(blindFindingStats([row("a", "inconclusive", [blind("r", 3)])], [row("a", "agree")])).toMatchObject({ before: 1, mapped: 0, stillBlind: 0 });
  });
});

describe("compareRuns", () => {
  it("is equal when only inconclusives resolve, and reports the control", () => {
    const base = [row("a", "inconclusive"), row("b", "agree")];
    const instr = [row("a", "agree", [], { control: { matched: true, diffs: [] } }), row("b", "agree", [], { control: { matched: false, diffs: ["diagnostics: x"] } })];
    const report = compareRuns(base, instr);
    expect(report).toMatchObject({ programs: 2, sameVerdict: 1, equal: true, control: { programs: 2, matched: 1, mismatches: [{ id: "b", diffs: ["diagnostics: x"] }] } });
    expect(report.resolved).toEqual([{ id: "a", from: "inconclusive", to: "agree" }]);
  });
  it("fails on any other movement", () => {
    const report = compareRuns([row("a", "agree"), row("b", "candidate-false-positive")], [row("a", "inconclusive"), row("b", "agree")]);
    expect(report.equal).toBe(false);
    expect(report.failures.map((f) => f.id)).toEqual(["a", "b"]);
  });
});
