import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { compareLines, comparePerf, compareUnit, incomparable, readRun } from "./perf-compare.mjs";

const unit = (label, extra = {}) => ({ label, programs: 1000, programsPerSecond: 50, cache: "runtime warm, typecheck warm", gridSample: null, checker: { p50: 100, p99: 800, overBudget: 3 }, ...extra });
const run = (units, extra = {}) => ({ version: 1, codes: "A,B", seed: "1", selection: { grids: "mx-recall", splitCodes: true }, mode: { warmChecker: false, roleLog: false }, budget: { ms: 1000, mb: 500 }, host: { cpus: 10 }, units, ...extra });

describe("the throughput comparison", () => {
  it("passes runs within the threshold and says the cache state", () => {
    const result = comparePerf(run([unit("mx-recall/A"), unit("mx-recall/B")]), run([unit("mx-recall/A", { programsPerSecond: 45 }), unit("mx-recall/B", { checker: { p50: 115, p99: 900, overBudget: 4 } })]));
    expect(result).toMatchObject({ comparable: true, worse: [] });
    const lines = compareLines(result);
    expect(lines[1]).toBe("perf-compare: cache runtime warm, typecheck warm in both runs");
    expect(lines).toContain("  mx-recall/A: programs/s 50 -> 45 (-10%); checker p50 ms 100 -> 100 (0%); checker p99 ms 800 -> 800 (0%); over budget 3 -> 3 (0%)");
    expect(lines[lines.length - 1]).toBe("perf-compare: nothing worse");
  });

  it("flags a metric more than the threshold worse", () => {
    const result = comparePerf(run([unit("u")]), run([unit("u", { programsPerSecond: 40, checker: { p50: 130, p99: 800, overBudget: 9 } })]));
    expect(result.worse).toEqual(["u: programs/s, checker p50 ms, over budget"]);
    expect(compareLines(result).join("\n")).toMatch(/programs\/s 50 -> 40 \(-20%\) WORSE/);
    // 50/40 - 1 = 25% more time per program; a configured 30% passes it.
    expect(comparePerf(run([unit("u")]), run([unit("u", { programsPerSecond: 40 })]), { threshold: 0.3 }).worse).toEqual([]);
  });

  it("ignores changes under the absolute floors", () => {
    const metrics = compareUnit(unit("u", { checker: { p50: 10, p99: 20, overBudget: 0 } }), unit("u", { checker: { p50: 25, p99: 39, overBudget: 1 } }), { threshold: 0.2, floorMs: 20, floorCount: 2 });
    expect(metrics.filter((m) => m.worse)).toEqual([]);
    const over = compareUnit(unit("u", { checker: { p50: 10, p99: 20, overBudget: 0 } }), unit("u", { checker: { p50: 10, p99: 41, overBudget: 2 } }), { threshold: 0.2, floorMs: 20, floorCount: 2 });
    expect(over.filter((m) => m.worse).map((m) => m.metric)).toEqual(["checker p99 ms", "over budget"]);
  });

  it("refuses runs that are not comparable, never passing them", () => {
    const base = run([unit("u")]);
    expect(incomparable(base, run([unit("u", { cache: "runtime cold, typecheck cold" })]))).toEqual(['u cache: "runtime warm, typecheck warm" vs "runtime cold, typecheck cold"']);
    expect(incomparable(base, run([unit("u", { programs: 999 })]))).toEqual(["u programs: 1000 vs 999"]);
    expect(incomparable(base, run([unit("u", { gridSample: { fraction: 0.1 } })]))[0]).toMatch(/^u sample/);
    expect(incomparable(base, run([unit("u")], { codes: "A" }))[0]).toMatch(/^codes/);
    expect(incomparable(base, run([unit("u")], { mode: { warmChecker: true, roleLog: false } }))[0]).toMatch(/^checker mode/);
    expect(incomparable(base, run([unit("v")]))[0]).toMatch(/^units/);
    const result = comparePerf(base, run([unit("u")], { host: { cpus: 8 } }));
    expect(result).toMatchObject({ comparable: false, worse: [] });
    expect(compareLines(result)).toContain("perf-compare: NOT COMPARABLE");
  });

  it("reads a run's perf.json and names a run without one", () => {
    const dir = mkdtempSync(join(tmpdir(), "fuzz-perf-compare-"));
    expect(() => readRun(dir)).toThrow(/no perf\.json/);
    writeFileSync(join(dir, "perf.json"), JSON.stringify(run([unit("u")])));
    expect(readRun(dir).units[0].label).toBe("u");
  });
});
