import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { cacheState, gridProfile, idleStretches, machineWindow, parseDiskstats, parseProcStat, perfLine, samplerUnavailable, startSampler, unitPerf } from "./perf.mjs";
import { parse, runPerf, runPerfLine } from "./focus.mjs";

const STAT = "cpu  100 5 50 800 20 1 2 0 0 0\ncpu0 50 2 25 400 10 0 1 0 0 0\ncpu1 50 3 25 400 10 1 1 0 0 0\nintr 1\n";
const DISKSTATS = [
  " 254       0 vda 100 0 2000 50 300 0 4096 70 0 100 120 0 0 0 0",
  " 254       1 vda1 90 0 1900 40 290 0 4000 60 0 90 100 0 0 0 0",
  "   7       0 loop0 1 0 8 0 1 0 999 0 0 0 0 0 0 0 0",
].join("\n");

// A sample every second: `idle` of each second's 100 jiffies idle (the rest busy), `iowait` of them waiting on the disk.
const series = (shares, { iowait = 0, sectorsPerSecond = 2048 } = {}) => {
  const out = [{ t: 0, busy: 0, idle: 0, iowait: 0, sectors: 0 }];
  for (const share of shares) {
    const last = out[out.length - 1];
    const idle = Math.round(share * 100);
    out.push({ t: last.t + 1000, busy: last.busy + 100 - idle, idle: last.idle + idle - iowait, iowait: last.iowait + iowait, sectors: last.sectors + sectorsPerSecond });
  }
  return out;
};

describe("the /proc readings", () => {
  it("read the CPU line and the whole disks' sectors written", () => {
    expect(parseProcStat(STAT)).toEqual({ busy: 158, idle: 800, iowait: 20, cores: 2 });
    expect(parseProcStat("intr 1")).toBeNull();
    // The partition and the loop device would count the same writes twice, or writes that never reach a disk.
    expect(parseDiskstats(DISKSTATS, new Set(["vda"]))).toBe(4096);
  });
});

describe("the idle stretches", () => {
  it("merge consecutive mostly idle intervals and keep the long ones", () => {
    const samples = series([0.1, ...Array(12).fill(0.9), 0.2, ...Array(5).fill(0.95), 0.1]);
    expect(idleStretches(samples, { minSeconds: 10 })).toEqual([{ atS: 1, seconds: 12, idle: 0.9, iowait: 0 }]);
    expect(idleStretches(samples, { minSeconds: 5 }).map((s) => s.seconds)).toEqual([12, 5]);
  });

  it("say how much of a stretch waited on the disk", () => {
    const [stretch] = idleStretches(series(Array(10).fill(0.8), { iowait: 40 }), { minSeconds: 10 });
    expect(stretch).toMatchObject({ seconds: 10, idle: 0.8, iowait: 0.4 });
  });

  it("give a window's CPU and disk use", () => {
    expect(machineWindow(series([0.5, 0.5]), { minSeconds: 10 })).toEqual({ cpuBusy: 0.5, iowait: 0, diskMiB: 2, idleStretches: [] });
    expect(machineWindow([])).toMatchObject({ cpuBusy: null, why: "no samples" });
  });
});

describe("the grid's phase clock", () => {
  it("sums each phase's sync and async work", async () => {
    let now = 0;
    const profile = gridProfile(() => now);
    profile.time("judge", () => { now += 5; });
    profile.time("judge", () => { now += 7; });
    await profile.time("chrome", async () => { now += 100; });
    profile.add("generate", 3.4);
    expect(profile.json({ programs: 2 })).toEqual({ wallMs: 112, phases: { judge: { ms: 12, n: 2 }, chrome: { ms: 100, n: 1 }, generate: { ms: 3, n: 1 } }, programs: 2, busy: undefined });
  });
});

describe("a unit's perf record", () => {
  it("joins the grid's profile, its cost summary, the machine and the fold", () => {
    const dir = mkdtempSync(join(tmpdir(), "fuzz-perf-"));
    writeFileSync(join(dir, "profile.json"), JSON.stringify({ wallMs: 9000, phases: { generate: { ms: 500, n: 1 }, judge: { ms: 200, n: 40 } }, busy: { checker: { ms: 30000, n: 40 } } }));
    writeFileSync(join(dir, "cost.json"), JSON.stringify({ budget: { ms: 1000, mb: 500 }, checkerMs: { p50: 100, p90: 200, p99: 900, max: 1200 }, maxRssMb: { max: 300 }, overBudget: [{ id: "x" }], decisions: null }));
    const machine = machineWindow(series(Array(12).fill(0.9)), { minSeconds: 10 });
    const perf = unitPerf({ label: "mx-recall/NO_OWNER_CONTEXT", dir, startedAt: 0, endedAt: 10_000, programs: 40, machine, fold: { foldMs: 1500, pruneMs: 300 }, cache: "runtime warm, typecheck warm" });
    expect(perf).toMatchObject({ programsPerSecond: 4, checker: { p50: 100, p99: 900, overBudget: 1 }, phases: { fold: { ms: 1500 }, prune: { ms: 300 } }, diskMiBPerProgram: 0.3 });
    expect(perfLine(perf)).toBe("perf mx-recall/NO_OWNER_CONTEXT: 40 programs in 10.0s = 4/s; generate 0.5s, judge 0.2s, fold 1.5s, prune 0.3s; busy checker 30.0s/40; checker p50 100 ms p99 900 ms, RSS max 300 MB, 1 over budget; cpu busy 10% (iowait 0%), 1 idle stretch (longest 12s), disk 12 MiB = 0.3 MiB/program");
    // Without the grid's files or a sampler the line says so.
    expect(unitPerf({ label: "u", dir, startedAt: 0, endedAt: 1, programs: 1, machine, fold: { foldMs: null, pruneMs: 40 } }).phases).not.toHaveProperty("fold");
    expect(perfLine(unitPerf({ label: "u", dir: mkdtempSync(join(tmpdir(), "fuzz-perf-")), startedAt: 0, endedAt: 1000, programs: 0, machine: null }))).toMatch(/no profile\.json; no cost\.json; machine not sampled/);
  });

  it("states each cache as none, cold, warm or mixed", () => {
    expect(cacheState(null)).toBe("none");
    expect(cacheState({ hits: 0, misses: 10, typecheck: { hits: 99, misses: 1 } })).toBe("runtime cold, typecheck warm");
    expect(cacheState({ hits: 50, misses: 50 })).toBe("runtime mixed, typecheck none");
  });
});

describe("the run's perf record", () => {
  it("names the unit an idle stretch fell in, or the gap between units", () => {
    const sampler = { samples: () => series([...Array(3).fill(0.1), ...Array(12).fill(0.95), ...Array(3).fill(0.1)]).map((s) => ({ ...s, t: s.t + 1_000_000 })) };
    const units = [{ label: "b", startedAt: 1_010_000, endedAt: 1_018_000, programs: 10 }, { label: "a", startedAt: 1_000_000, endedAt: 1_003_000, programs: 20 }];
    const report = runPerf({ options: parse(["--codes", "A"]), out: "/o", units, sampler, runStarted: 1_000_000, runEnded: 1_018_000, stretchOptions: { minSeconds: 10 } });
    expect(report.units.map((u) => u.label)).toEqual(["a", "b"]);
    expect(report.machine.idleStretches).toEqual([{ atS: 3, seconds: 12, idle: 0.95, iowait: 0, during: "between units" }]);
    expect(report).toMatchObject({ codes: "A", programs: 30, mode: { warmChecker: false, roleLog: false }, budget: { ms: 1000, mb: 500 } });
    expect(runPerfLine(report)).toMatch(/^perf: 30 programs in 18s = 1\.67\/s over 2 units; cpu busy \d+% \(iowait 0%\), disk [\d.]+ MiB, 1 idle stretch$/);
  });

  it("takes the focus options it needs", () => {
    expect(parse(["--codes", "A", "--budget-ms", "250", "--perf-against", "/r", "--perf-threshold", "0.1", "--idle-stretch-s", "5"])).toMatchObject({ budgetMs: "250", perfAgainst: "/r", perfThreshold: "0.1", idleStretchS: "5" });
    expect(() => parse(["--codes", "A", "--perf-threshold", "0.1"])).toThrow(/needs --perf-against/);
    expect(() => parse(["--codes", "A", "--budget-mb", "x"])).toThrow(/--budget-mb takes a non-negative number/);
  });
});

describe("the sampler", () => {
  it("samples on Linux and is null elsewhere", async () => {
    const sampler = startSampler({ intervalMs: 20 });
    if (samplerUnavailable()) { expect(sampler).toBeNull(); return; }
    const from = Date.now();
    await new Promise((resolve) => setTimeout(resolve, 200));
    const samples = sampler.samples(from, Date.now());
    await sampler.stop();
    expect(samples.length).toBeGreaterThan(2);
    expect(samples[samples.length - 1].busy + samples[samples.length - 1].idle).toBeGreaterThan(samples[0].busy + samples[0].idle);
  });
});
