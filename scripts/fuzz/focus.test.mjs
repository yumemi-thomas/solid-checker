import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { spawnSync } from "node:child_process";
import { DEFAULT_GRIDS, GRIDS, HOST_LIMIT, diskFilled, hostArgs, hostLimits, noRoom, parse, readGrid, roleLogPath, roomAt, runGridProcess, unitPlaces, unitProblems } from "./focus.mjs";

describe("hostLimits", () => {
  it("caps every pool by default, on any machine, and takes every core only when named", () => {
    expect(hostLimits({}, 14)).toEqual(["--concurrency", String(HOST_LIMIT), "--workers", String(HOST_LIMIT), "--chrome-concurrency", String(HOST_LIMIT)]);
    expect(hostLimits({}, 4)).toEqual(["--concurrency", "4", "--workers", "4", "--chrome-concurrency", "4"]);
    expect(hostLimits({ allCores: true }, 14)).toEqual(["--all-cores"]);
    expect(hostArgs()).not.toContain("--all-cores");
    expect(parse(["--codes", "A", "--all-cores"]).allCores).toBe(true);
    expect(parse(["--codes", "A"]).allCores).toBeUndefined();
  });
});

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
    expect(readGrid(join(out, "missing"))).toEqual({ outcomes: {}, flagged: [], oracle: null, behaviour: [], gridSample: null, gridShard: null, selected: null, partial: false });
    // A sampled grid (`--sample-big`) and a shard (`--shard`) say so in their report, with the programs the grid selected.
    writeFileSync(join(out, "legal.json"), JSON.stringify({ gridSample: { fraction: 0.1, kept: 5, of: 50 }, gridShard: { index: 1, count: 4, kept: 2, of: 5 }, selected: 2 }));
    expect(readGrid(out)).toMatchObject({ gridSample: { fraction: 0.1, kept: 5, of: 50 }, gridShard: { index: 1, count: 4, kept: 2, of: 5 }, selected: 2 });
    // A grid with no shape for its codes still writes its report: selected 0, which is not a report that is missing.
    writeFileSync(join(out, "legal.json"), JSON.stringify({ selected: 0 }));
    expect(readGrid(out).selected).toBe(0);
  });

  it("flags a grid that judged nothing unless it finished and selected no program", () => {
    const ok = { status: 0, signal: null, timedOut: false };
    const problems = (run, grid) => unitProblems("u", { run, grid, total: 0, log: "", logPath: "u.log", timeoutMinutes: 1 });
    expect(problems(ok, { selected: 0 })).toEqual([]);
    expect(problems(ok, { selected: null })).toEqual(["u: no program was judged (u.log)"]);
    expect(problems({ status: 1, signal: null, timedOut: false }, { selected: 0 })).toEqual(["u: exited 1 (u.log): ", "u: no program was judged (u.log)"]);
    expect(problems(ok, { selected: 0, partial: false })).toHaveLength(1);
  });

  it("runs the six grids of the full run by default, the recall and pair grids by name", () => {
    expect(DEFAULT_GRIDS).toEqual(["mx-base", "mx-src", "mx-combo", "lg-base", "lg-src", "lg-combo", "mx-census"]);
    expect(Object.keys(GRIDS).sort()).toEqual([...DEFAULT_GRIDS, "mx-recall", "lg-recall", "mx-pairs", "lg-pairs"].sort());
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

describe("where a unit's files go, and whether they fit", () => {
  const GIB = 2 ** 30;
  /** A statfs over a map of path prefix to free GiB: the longest prefix that exists answers; the others throw (not created yet). */
  const disks = (free, exist = Object.keys(free)) => (path) => {
    const key = exist.filter((name) => path === name || path.startsWith(`${name}/`)).sort((a, b) => b.length - a.length)[0];
    if (!key) throw Object.assign(new Error("ENOENT"), { code: "ENOENT" });
    return { bavail: free[key] * GIB / 4096, bsize: 4096, files: 1000, ffree: 900 };
  };

  it("puts the role log beside the grid output, on its disk, whatever the scratch is", () => {
    expect(roleLogPath("/data/runs/lg-recall/CODE")).toBe("/data/runs/lg-recall/CODE.roles.jsonl");
    expect(roleLogPath("/data/runs/lg-base")).toBe("/data/runs/lg-base.roles.jsonl");
  });

  it("checks the output's disk (where the role log goes) and the scratch, naming the one that is full", () => {
    const places = unitPlaces({ unitDir: "/data/runs/lg-recall/CODE", scratch: "/ram/scratch/focus-1", roleLog: true });
    expect(places).toEqual([["grid output and role log", "/data/runs/lg-recall"], ["scratch", "/ram/scratch/focus-1"]]);
    expect(unitPlaces({ unitDir: "/d/g", roleLog: false })).toEqual([["grid output", "/d"]]);
    const statfs = disks({ "/data": 100, "/ram": 16 });
    expect(noRoom(places, { statfs })).toBeNull();
    // A tmpfs the role log had filled: the reason names the scratch and its path.
    expect(noRoom(places, { statfs: disks({ "/data": 100, "/ram": 1.5 }) })).toBe("scratch (/ram/scratch/focus-1): 1.5 GiB free");
    expect(noRoom(places, { statfs: disks({ "/data": 2, "/ram": 1.5 }) })).toBe("grid output and role log (/data/runs/lg-recall): 2.0 GiB free");
    // The floor is a parameter: 20 GiB is met by the data disk and missed by the 16 GiB tmpfs.
    expect(noRoom(places, { statfs: disks({ "/data": 100, "/ram": 16 }), minGiB: 20 })).toBe("scratch (/ram/scratch/focus-1): 16.0 GiB free");
  });

  it("measures a directory not created yet at its nearest existing ancestor, and nothing it cannot find", () => {
    const places = unitPlaces({ unitDir: "/data/runs/lg-recall/CODE", scratch: "/ram/scratch/focus-1", roleLog: true });
    // Neither /data/runs nor /ram/scratch/focus-1 exists on the first unit: the disks are /data's and /ram's.
    const statfs = disks({ "/data": 1, "/ram": 16 }, ["/data", "/ram"]);
    expect(noRoom(places, { statfs })).toBe("grid output and role log (/data/runs/lg-recall): 1.0 GiB free");
    expect(roomAt("/nowhere/at/all", { statfs: disks({}) })).toBeNull();
  });

  it("recognises a disk that filled during a unit, in its log or in its rows", () => {
    expect(diskFilled({ log: "matrix: stopped: ENOSPC: no space left on device, open 'cases/oracle.json'" })).toBe(true);
    expect(diskFilled({ log: "", flagged: [{ outcome: "harness", detail: { error: "ENOSPC: no space left on device, write" } }] })).toBe(true);
    expect(diskFilled({ log: "ok", flagged: [{ outcome: "harness", detail: { error: "Chrome exited" } }] })).toBe(false);
  });

  it("stops a run before its first unit when the scratch or output has no room, with a reason and no harness rows", () => {
    const root = mkdtempSync(join(tmpdir(), "fuzz-focus-room-"));
    try {
      const out = join(root, "out");
      const run = spawnSync(process.execPath, [resolve(import.meta.dirname, "focus.mjs"), "--codes", "NO_OWNER_CLEANUP", "--grids", "mx-base", "--role-log", "--scratch", join(root, "scratch"), "--out", out],
        { encoding: "utf8", env: { ...process.env, SOLID_CHECKER_FUZZ_MIN_FREE_GIB: "1000000000", SOLID_CHECKER_FUZZ_PERF: "0" } });
      expect(run.status).toBe(2);
      expect(run.stdout).toMatch(/needs a look \(1\):/);
      expect(run.stdout).toMatch(/stopped: mx-base: not started, grid output and role log \(.*\): .* GiB free/);
      expect(run.stdout).not.toMatch(/harness/);
      expect(() => readFileSync(join(out, "mx-base.log"))).toThrow();
    } finally { rmSync(root, { recursive: true, force: true }); }
  }, 60_000);
});
