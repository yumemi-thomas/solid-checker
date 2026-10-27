import { describe, expect, it } from "vitest";
import { configFrom, defaultParallelism, parseArgs } from "./run.mjs";

describe("parseArgs", () => {
  it("has defaults", () => {
    expect(parseArgs([])).toMatchObject({ seed: 1, count: 20, shrinkMode: "discovery", budget: 120 });
  });
  it("reads values and flags", () => {
    expect(parseArgs(["--seed", "7", "--count", "5", "--out", "/tmp/x", "--confirm-with-chrome", "--shrink", "--shrink-mode", "focused", "--checker-bin", "/b", "--fast-install", "/f"]))
      .toMatchObject({ seed: 7, count: 5, out: "/tmp/x", confirmWithChrome: true, shrink: true, shrinkMode: "focused", checkerBin: "/b", fastInstall: "/f" });
  });
  it("refuses what it does not know", () => {
    expect(() => parseArgs(["--bogus"])).toThrow(/unknown option/);
    expect(() => parseArgs(["--seed"])).toThrow(/needs a value/);
    expect(() => parseArgs(["--count", "x"])).toThrow(/non-negative integer/);
    expect(() => parseArgs(["stray"])).toThrow(/unexpected argument/);
    expect(() => parseArgs(["--shrink-mode", "wild"])).toThrow(/discovery or focused/);
  });
});

describe("parallelism", () => {
  it("uses half the cores by default and every core with --all-cores", () => {
    expect(defaultParallelism(14)).toBe(7);
    expect(defaultParallelism(1)).toBe(1);
    expect(defaultParallelism(14, { allCores: true })).toBe(14);
    expect(parseArgs(["--all-cores", "--shrink-jobs", "3", "--shrink-batch", "5"])).toMatchObject({ allCores: true, shrinkJobs: 3, shrinkBatch: 5 });
    const all = configFrom(parseArgs(["--all-cores"]), "out");
    const half = configFrom(parseArgs([]), "out");
    expect(all.concurrency).toBeGreaterThanOrEqual(half.concurrency);
    expect(configFrom(parseArgs(["--all-cores", "--workers", "2"]), "out").workers).toBe(2);
  });
});
