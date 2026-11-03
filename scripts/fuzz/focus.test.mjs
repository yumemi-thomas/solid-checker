import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { DEFAULT_GRIDS, GRIDS, parse, readGrid, roomAt, runGridProcess } from "./focus.mjs";

describe("readGrid", () => {
  it("tallies the outcomes, flags what needs a look, and counts every difference from the oracle as behaviour", () => {
    const out = mkdtempSync(join(tmpdir(), "fuzz-focus-"));
    mkdirSync(out, { recursive: true });
    const rows = [{ id: "a", outcome: "agree" }, { id: "b", outcome: "false-clean", detail: { line: 3 } }, { id: "c", outcome: "recall-gap" }, { id: "d", outcome: "false-positive" }];
    writeFileSync(join(out, "programs.jsonl"), rows.map((row) => JSON.stringify(row)).join("\n"));
    writeFileSync(join(out, "runtime-diff.json"), JSON.stringify({
      summary: { compared: 3, behaviour: 1, attributionOnly: 1, inconclusive: 0 },
      differing: [{ id: "x", diffs: [{ kind: "moved", detail: "C happy@1 chrome@2" }] }, { id: "y", diffs: [{ kind: "chrome-only", detail: "C@4" }] }],
    }));
    const grid = readGrid(out);
    expect(grid.outcomes).toEqual({ agree: 1, "false-clean": 1, "recall-gap": 1, "false-positive": 1 });
    expect(grid.flagged.map((row) => row.id)).toEqual(["b", "d"]);
    expect(grid.behaviour.map((result) => result.id)).toEqual(["x", "y"]);
    expect(readGrid(join(out, "missing"))).toEqual({ outcomes: {}, flagged: [], oracle: null, behaviour: [], gridSample: null });
    // A sampled grid (`--sample-big`) says so in its report.
    writeFileSync(join(out, "legal.json"), JSON.stringify({ gridSample: { fraction: 0.1, kept: 5, of: 50 } }));
    expect(readGrid(out).gridSample).toEqual({ fraction: 0.1, kept: 5, of: 50 });
  });

  it("runs the six grids of the full run by default, the recall and pair grids by name", () => {
    expect(DEFAULT_GRIDS).toEqual(["mx-base", "mx-src", "mx-combo", "lg-base", "lg-src", "lg-combo"]);
    expect(Object.keys(GRIDS)).toEqual([...DEFAULT_GRIDS, "mx-recall", "lg-recall", "mx-pairs", "lg-pairs"]);
  });
});

describe("the grid process", () => {
  it("parses the run options, and refuses splitting a smoke run by code", () => {
    const options = parse(["--codes", "all", "--split-codes", "--prune-cases", "--grid-timeout", "5", "--heap-mb", "2048"]);
    expect(options).toMatchObject({ codes: "all", splitCodes: true, pruneCases: true, gridTimeout: "5", heapMb: "2048" });
    expect(() => parse(["--smoke", "--split-codes"])).toThrow(/--split-codes/);
  });

  it("streams the process's output to its log while it runs, passes the heap, and kills it at the deadline", () => {
    const dir = mkdtempSync(join(tmpdir(), "fuzz-focus-run-"));
    const script = join(dir, "grid.mjs");
    writeFileSync(script, `
      import { writeFileSync } from "node:fs";
      import v8 from "node:v8";
      console.log("heap", Math.round(v8.getHeapStatistics().heap_size_limit / 1048576));
      if (process.argv[2] === "hang") setInterval(() => {}, 1000);
    `);
    const ok = runGridProcess(script, ["done"], { logPath: join(dir, "ok.log"), timeoutMs: 20_000, heapMb: 3000, runtime: "node" });
    expect(ok).toMatchObject({ status: 0, timedOut: false });
    const heap = Number(readFileSync(join(dir, "ok.log"), "utf8").match(/heap (\d+)/)[1]);
    expect(heap).toBeGreaterThanOrEqual(2900);
    const hung = runGridProcess(script, ["hang"], { logPath: join(dir, "hung.log"), timeoutMs: 1500, runtime: "node" });
    expect(hung.timedOut).toBe(true);
    expect(readFileSync(join(dir, "hung.log"), "utf8")).toMatch(/^heap/);
  }, 30_000);
});

describe("roomAt", () => {
  const disk = (over) => () => ({ bavail: 100 * 2 ** 20, bsize: 4096, files: 1000, ffree: 500, ...over });
  it("is null with room, and names a full disk or exhausted inodes", () => {
    expect(roomAt("/x", { statfs: disk({}) })).toBeNull();
    expect(roomAt("/x", { statfs: disk({ bavail: 2 ** 20 }) })).toMatch(/GiB free/);
    expect(roomAt("/x", { statfs: disk({ ffree: 10 }) })).toMatch(/10 of 1000 inodes free/);
    expect(roomAt("/x", { statfs: () => { throw new Error("no such dir"); } })).toBeNull();
  });
});
