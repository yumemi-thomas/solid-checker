import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { DEFAULT_BUDGET, addDecisions, budgetOf, costLine, costLines, costOf, costSummary, distribution, readCostLines, writeCosts } from "./cost.mjs";
import { foldDecisions } from "./roles.mjs";

const line = (id, checkerMs, extra = {}) => ({ id, code: "NO_OWNER_CONTEXT", context: "timer", layer: "direct", variant: "effect", checkerMs, cpuMs: checkerMs, maxRssKb: 100 * 1024, writeKb: 0, ...extra });

describe("a checker answer's cost", () => {
  it("takes the wall time and the executor's rusage", () => {
    expect(costOf({ wallMs: 120, usage: { cpuMs: 90, maxRssKb: 51200, writeKb: 4 } })).toEqual({ checkerMs: 120, cpuMs: 90, maxRssKb: 51200, writeKb: 4 });
  });

  it("records what it lacks as null with the reason, never zero", () => {
    expect(costOf({ wallMs: 120, usage: { cpuMs: null, maxRssKb: null, writeKb: null, usageWhy: "warm daemon" } })).toEqual({ checkerMs: 120, cpuMs: null, maxRssKb: null, writeKb: null, why: "warm daemon" });
    expect(costOf({ wallMs: 120 })).toMatchObject({ checkerMs: 120, maxRssKb: null, why: expect.stringMatching(/not the Rust executor/) });
    expect(costOf({ spawnError: "ENOENT", wallMs: 0 })).toMatchObject({ checkerMs: null, why: expect.stringMatching(/did not start/) });
    expect(costOf(null)).toMatchObject({ checkerMs: null, why: "no checker run" });
  });

  it("puts a row's identity and cell beside the value's cost", () => {
    const row = { id: "mx-a", code: "C", context: "x", layer: "direct", variant: "v", outcome: "agree", detail: {} };
    expect(costLine(row, { cost: { checkerMs: 5, cpuMs: 4, maxRssKb: 1, writeKb: 0 } })).toEqual({ id: "mx-a", code: "C", context: "x", layer: "direct", variant: "v", checkerMs: 5, cpuMs: 4, maxRssKb: 1, writeKb: 0 });
    expect(costLine(row, { error: "boom" })).toMatchObject({ checkerMs: null, why: "harness error" });
    // A type error skips the checker: its value has no cost.
    expect(costLine(row, { typecheck: { ok: false } })).toMatchObject({ checkerMs: null, why: "no checker run" });
  });
});

describe("the cost summary", () => {
  it("gives quantiles of the measured values only", () => {
    expect(distribution([3, null, 1, 2, 4])).toEqual({ n: 4, p50: 2, p90: 4, p99: 4, max: 4, total: 10 });
    expect(distribution([])).toEqual({ n: 0, p50: null, p90: null, p99: null, max: null, total: 0 });
  });

  it("ranks the slowest cells and lists every program over the budget", () => {
    const lines = [line("a", 100), line("b", 300), line("c", 1500, { layer: "iife" }), line("d", 200, { maxRssKb: 600 * 1024 }), line("e", null, { why: "harness error" })];
    const summary = costSummary(lines, DEFAULT_BUDGET);
    expect(summary.slowestCells[0]).toMatchObject({ layer: "iife", meanMs: 1500, programs: 1, share: 0.7143 });
    expect(summary.slowestCells[1]).toMatchObject({ layer: "direct", meanMs: 200, maxMs: 300, programs: 3 });
    expect(summary.overBudget.map((p) => p.id)).toEqual(["c", "d"]);
    expect(summary.overBudget[1]).toMatchObject({ maxRssMb: 600 });
    expect(summary.unmeasured).toEqual({ "harness error": 1 });
    expect(summary.decisions).toBeNull();
    const printed = costLines(summary, { prefix: "matrix" });
    expect(printed[0]).toMatch(/^matrix: checker 4\/5 programs timed: p50 200 ms.*2 over budget \(1000 ms or 500 MB\)$/);
    expect(printed.some((text) => /slow cell NO_OWNER_CONTEXT timer iife effect: mean 1500 ms, max 1500 ms over 1 \(71\.43% of checker time\)/.test(text))).toBe(true);
  });

  it("takes its budget from the options, positive only", () => {
    expect(budgetOf({})).toEqual({ ms: 1000, mb: 500 });
    expect(budgetOf({ budgetMs: "250", budgetMb: "64" })).toEqual({ ms: 250, mb: 64 });
    expect(() => budgetOf({ budgetMs: "0" })).toThrow(/positive/);
    expect(costSummary([line("a", 300)], budgetOf({ budgetMs: "250" })).overBudget.map((p) => p.id)).toEqual(["a"]);
  });
});

describe("the role decisions per program", () => {
  it("are counted by the fold, deduplicated, and added to the cost lines with the twin's", () => {
    const work = "/i/.solid-checker-fuzz/run-1/slots/3";
    const decision = (program, s) => ({ k: "read", f: `${work}/${program}/App.tsx`, s, e: s + 3, arm: "x.y", arms: ["x.y"] });
    const perProgram = new Map();
    foldDecisions([decision("mx-a", 1), decision("mx-a", 1), decision("mx-a", 9), decision("mx-a-run", 1), decision("mx-b", 2)], null, { perProgram });
    expect(Object.fromEntries(perProgram)).toEqual({ "mx-a": 2, "mx-a-run": 1, "mx-b": 1 });
    const dir = mkdtempSync(join(tmpdir(), "fuzz-cost-"));
    writeCosts(dir, [line("mx-a", 10), line("mx-b", 20), line("mx-c", 30)], { ms: 15, mb: 500 });
    const summary = addDecisions(dir, perProgram);
    expect(readCostLines(dir).map((l) => l.decisions)).toEqual([3, 1, 0]);
    // The budget the grid wrote is kept.
    expect(summary.budget).toEqual({ ms: 15, mb: 500 });
    expect(JSON.parse(readFileSync(join(dir, "cost.json"), "utf8")).decisions).toMatchObject({ n: 3, max: 3 });
    expect(summary.overBudget.map((p) => [p.id, p.decisions])).toEqual([["mx-c", 0], ["mx-b", 1]]);
  });

  it("leave a grid without cost lines alone", () => {
    const dir = mkdtempSync(join(tmpdir(), "fuzz-cost-"));
    writeFileSync(join(dir, "programs.jsonl"), "");
    expect(addDecisions(dir, new Map())).toBeNull();
  });
});
