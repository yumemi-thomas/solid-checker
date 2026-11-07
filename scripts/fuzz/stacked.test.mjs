import { existsSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { codeRules, codesForRule } from "./codes.mjs";
import { STACKED_PAIRS, pairOf, stackedProblems, stackedSplit } from "./stacked.mjs";

const here = new URL(".", import.meta.url).pathname;

describe("stacked claims (ADR 0887)", () => {
  it("every pair names a Solid code, a rule with runtime codes, and twins that exist", () => {
    expect(Object.keys(STACKED_PAIRS).length).toBeGreaterThan(0);
    for (const [pair, { patterns, why }] of Object.entries(STACKED_PAIRS)) {
      const [first, rule] = pair.split("|");
      expect(codeRules[first], pair).toBeDefined();
      expect(codesForRule(rule).length, pair).toBeGreaterThan(0);
      expect(codesForRule(rule), `${pair}: the first code is the rule's own, so nothing is stacked`).not.toContain(first);
      expect(why, pair).toBeTruthy();
      for (const id of patterns) expect(existsSync(`${here}patterns/${id}/App.tsx`), `${pair}: ${id}`).toBe(true);
    }
  });
  it("splits a stacked row by its pair, and an unproved pair is a false positive", () => {
    const pairs = { "A|r": { patterns: [] } };
    const judged = [{ finding: "r@3", verdict: "stacked", first: "A" }, { finding: "s@4", verdict: "stacked", first: "A" }, { finding: "t@5", verdict: "false-positive" }, { finding: "u@6", verdict: "unexecuted" }];
    const { wrong, stacked } = stackedSplit(judged, pairs);
    expect(stacked.map(pairOf)).toEqual(["A|r"]);
    expect(wrong.map((row) => row.finding)).toEqual(["s@4", "t@5"]);
  });
  it("a pair's proof fails when its twin is missing, not fine, or Chrome reported no code of the rule", () => {
    const pairs = { "PRIMITIVE_IN_EFFECT_CALLBACK|missing-owner": { patterns: ["x/a"] } };
    expect(stackedProblems([], new Map(), pairs)).toEqual(["PRIMITIVE_IN_EFFECT_CALLBACK|missing-owner: no pattern x/a"]);
    expect(stackedProblems([{ id: "x/a", outcome: "fine" }], new Map([["x/a", { codes: ["NO_OWNER_CONTEXT"] }]]), pairs)).toEqual([]);
    expect(stackedProblems([{ id: "x/a", outcome: "false-clean" }], new Map([["x/a", { codes: ["CONTEXT_NOT_FOUND"] }]]), pairs)).toEqual(["PRIMITIVE_IN_EFFECT_CALLBACK|missing-owner: x/a is false-clean, not fine", "PRIMITIVE_IN_EFFECT_CALLBACK|missing-owner: Chrome reported no code of missing-owner in x/a"]);
    expect(stackedProblems([], new Map(), pairs, { only: "y/" })).toEqual([]);
  });
});
