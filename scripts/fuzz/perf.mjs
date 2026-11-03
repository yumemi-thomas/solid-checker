// The harness's self-profile (measurement only: nothing here reads or writes a row or a verdict).
//
// A grid (matrix.mjs, legal.mjs) writes `<grid>/profile.json`: its wall time split into phases (generate, evaluate, judge, Chrome
// confirmation, runtime differential, shrink, write) and the pipeline's busy totals (materialize, type check, checker, fast path:
// pipeline.mjs `phaseTimings`, summed over overlapping work items, so they can exceed the wall time).
//
// focus.mjs samples the machine while it runs (`startSampler`, a worker thread, since the grids run under spawnSync): every 250 ms the
// CPU counters of /proc/stat and the whole disks' sectors written of /proc/diskstats, into a shared buffer. Per grid unit it adds the
// CPU busy and iowait share, the stretches of mostly idle cores (`idleStretches`: at least 75% idle for at least 10 s, such as the
// 12-minute fold gap of full-8), the disk MiB written per program (the VM's block devices, so aggregate, not per program precision:
// other processes' writes count too, tmpfs writes never do), and the fold and prune times of fold-worker.mjs. Off Linux the sampler
// records nothing and the fields are null with the reason.
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { Worker } from "node:worker_threads";

/** A grid's phase clock: `time(phase, work)` adds the work's wall time (sync or async) to the phase; `json(extra)` the record. */
export function gridProfile(now = () => performance.now()) {
  const started = now();
  const phases = {};
  const add = (phase, ms) => { const slot = (phases[phase] ??= { ms: 0, n: 0 }); slot.ms += ms; slot.n += 1; };
  const time = (phase, work) => {
    const start = now();
    const value = work();
    if (value && typeof value.then === "function") return value.finally(() => add(phase, now() - start));
    add(phase, now() - start);
    return value;
  };
  const round = (record) => Object.fromEntries(Object.entries(record).map(([phase, { ms, n }]) => [phase, { ms: Math.round(ms), n }]));
  return { add, time, json: (extra = {}) => ({ wallMs: Math.round(now() - started), phases: round(phases), ...extra, busy: extra.busy ? round(extra.busy) : undefined }) };
}

/** /proc/stat's aggregate CPU line: busy, idle and iowait jiffies, and the core count. */
export function parseProcStat(text) {
  const lines = text.split("\n");
  const cpu = lines.find((line) => line.startsWith("cpu "));
  if (!cpu) return null;
  const [user, nice, system, idle, iowait = 0, irq = 0, softirq = 0, steal = 0] = cpu.trim().split(/\s+/).slice(1).map(Number);
  return { busy: user + nice + system + irq + softirq + steal, idle, iowait, cores: lines.filter((line) => /^cpu\d+ /.test(line)).length };
}

/** Sectors (512 bytes) written to the named whole disks, from /proc/diskstats. */
export function parseDiskstats(text, disks) {
  let sectors = 0;
  for (const line of text.split("\n")) {
    const fields = line.trim().split(/\s+/);
    if (fields.length >= 10 && disks.has(fields[2])) sectors += Number(fields[9]);
  }
  return sectors;
}

/** The whole block disks (/sys/block), less loop, RAM, device-mapper and md devices, which repeat or never reach a disk. */
export function wholeDisks(root = "/sys/block") {
  try { return new Set(readdirSync(root).filter((name) => !/^(loop|ram|zram|dm-|md|sr|nbd)/.test(name))); } catch { return new Set(); }
}

/** Why this machine cannot be sampled, or null. */
export const samplerUnavailable = () => (existsSync("/proc/stat") && existsSync("/proc/diskstats") ? null : "no /proc/stat or /proc/diskstats (not Linux)");

// perf-sampler.mjs writes the same layout.
export const FIELDS = 5; // t, busy, idle, iowait, sectors written
export const CAPACITY = 1 << 17; // ~9 hours at 250 ms

/**
 * Starts the sampler: a worker that writes a sample every `intervalMs` into a shared ring. `samples(from, to)` reads the samples
 * between two `Date.now()` times without waiting on the worker; `stop()` ends it. Null off Linux (`samplerUnavailable`).
 */
export function startSampler({ intervalMs = 250 } = {}) {
  if (samplerUnavailable()) return null;
  const head = new SharedArrayBuffer(8);
  const ring = new SharedArrayBuffer(CAPACITY * FIELDS * 8);
  const worker = new Worker(new URL("./perf-sampler.mjs", import.meta.url), { workerData: { head, ring, intervalMs, disks: [...wholeDisks()] } });
  worker.unref();
  const count = new Int32Array(head);
  const values = new Float64Array(ring);
  return {
    samples(from = 0, to = Infinity) {
      const n = Atomics.load(count, 0);
      const out = [];
      for (let i = Math.max(0, n - CAPACITY); i < n; i += 1) {
        const at = (i % CAPACITY) * FIELDS;
        const t = values[at];
        if (t >= from && t <= to) out.push({ t, busy: values[at + 1], idle: values[at + 2], iowait: values[at + 3], sectors: values[at + 4] });
      }
      return out;
    },
    stop: () => worker.terminate(),
  };
}

/**
 * The stretches of mostly idle cores in `samples`: consecutive intervals with at least `idleShare` of the CPU idle (idle + iowait),
 * merged, at least `minSeconds` long. Each says when it started (ms since `origin`), how long it lasted, and its idle and iowait
 * shares: a high iowait share is blocked on the disk, not lacking work.
 */
export function idleStretches(samples, { idleShare = 0.75, minSeconds = 10, origin = samples[0]?.t ?? 0 } = {}) {
  const stretches = [];
  let open = null;
  const close = () => {
    if (open && (open.end - open.start) / 1000 >= minSeconds) {
      const total = open.busy + open.idle + open.iowait;
      stretches.push({ atS: Math.round((open.start - origin) / 1000), seconds: Math.round((open.end - open.start) / 1000), idle: Math.round(((open.idle + open.iowait) / total) * 100) / 100, iowait: Math.round((open.iowait / total) * 100) / 100 });
    }
    open = null;
  };
  for (let i = 1; i < samples.length; i += 1) {
    const [a, b] = [samples[i - 1], samples[i]];
    const busy = b.busy - a.busy, idle = b.idle - a.idle, iowait = b.iowait - a.iowait;
    const total = busy + idle + iowait;
    if (total <= 0) continue;
    if ((idle + iowait) / total >= idleShare) {
      open ??= { start: a.t, busy: 0, idle: 0, iowait: 0 };
      open.end = b.t; open.busy += busy; open.idle += idle; open.iowait += iowait;
    } else close();
  }
  close();
  return stretches;
}

/** The machine over a window of samples: CPU busy and iowait shares, MiB written to the disks, and its idle stretches. */
export function machineWindow(samples, options = {}) {
  if (samples.length < 2) return { cpuBusy: null, iowait: null, diskMiB: null, idleStretches: [], why: samples.length ? "one sample" : "no samples" };
  const [first, last] = [samples[0], samples[samples.length - 1]];
  const busy = last.busy - first.busy, idle = last.idle - first.idle, iowait = last.iowait - first.iowait;
  const total = busy + idle + iowait;
  return {
    cpuBusy: total > 0 ? Math.round((busy / total) * 100) / 100 : null,
    iowait: total > 0 ? Math.round((iowait / total) * 100) / 100 : null,
    diskMiB: Math.round(((last.sectors - first.sectors) * 512 / 2 ** 20) * 100) / 100,
    idleStretches: idleStretches(samples, { origin: first.t, ...options }),
  };
}

/**
 * One grid unit's perf record (focus.mjs): its programs and wall time, throughput, the grid's phases (`profile.json`), the checker
 * cost summary (`cost.json`), the machine window, and the fold and prune times. `perf.json` beside the unit's results is this.
 */
export function unitPerf({ label, dir, startedAt, endedAt, programs, machine, fold = null, cache = null, gridSample = null, gridShard = null }) {
  const read = (name) => (existsSync(join(dir, name)) ? JSON.parse(readFileSync(join(dir, name), "utf8")) : null);
  const profile = read("profile.json");
  const cost = read("cost.json");
  const wallMs = endedAt - startedAt;
  return {
    label, programs, wallMs,
    programsPerSecond: wallMs > 0 ? Math.round((programs / (wallMs / 1000)) * 100) / 100 : null,
    // A fold or a prune that did not run (no --role-log, no --prune-cases) is left out, not timed as zero.
    phases: profile ? { ...profile.phases, ...(fold?.foldMs != null ? { fold: { ms: fold.foldMs } } : {}), ...(fold?.pruneMs != null ? { prune: { ms: fold.pruneMs } } : {}) } : null,
    busy: profile?.busy ?? null,
    checker: cost ? { p50: cost.checkerMs.p50, p90: cost.checkerMs.p90, p99: cost.checkerMs.p99, max: cost.checkerMs.max, maxRssMb: cost.maxRssMb.max, overBudget: cost.overBudget.length, budget: cost.budget, decisions: cost.decisions } : null,
    machine: machine ?? { why: samplerUnavailable() ?? "not sampled" },
    diskMiBPerProgram: machine?.diskMiB != null && programs ? Math.round((machine.diskMiB / programs) * 1000) / 1000 : null,
    cache, gridSample, gridShard,
  };
}

const s = (ms) => (typeof ms === "number" ? `${(ms / 1000).toFixed(1)}s` : "-");
/** A unit's perf record as one line. */
export function perfLine(perf) {
  const phases = perf.phases ? ["generate", "evaluate", "judge", "chrome", "runtimeDiff", "shrink", "write", "fold", "prune"].filter((phase) => perf.phases[phase]).map((phase) => `${phase} ${s(perf.phases[phase].ms)}`).join(", ") : "no profile.json";
  const busy = perf.busy ? Object.entries(perf.busy).map(([phase, { ms, n }]) => `${phase} ${s(ms)}/${n}`).join(", ") : "";
  const checker = perf.checker ? `checker p50 ${perf.checker.p50 ?? "-"} ms p99 ${perf.checker.p99 ?? "-"} ms, RSS max ${perf.checker.maxRssMb ?? "-"} MB, ${perf.checker.overBudget} over budget` : "no cost.json";
  const m = perf.machine;
  const machine = m?.cpuBusy != null ? `cpu busy ${Math.round(m.cpuBusy * 100)}% (iowait ${Math.round(m.iowait * 100)}%), ${m.idleStretches.length} idle stretch${m.idleStretches.length === 1 ? "" : "es"}${m.idleStretches.length ? ` (longest ${Math.max(...m.idleStretches.map((x) => x.seconds))}s)` : ""}, disk ${m.diskMiB} MiB = ${perf.diskMiBPerProgram ?? "-"} MiB/program` : `machine not sampled (${m?.why ?? "?"})`;
  return `perf ${perf.label}: ${perf.programs} programs in ${s(perf.wallMs)} = ${perf.programsPerSecond ?? "-"}/s; ${phases}${busy ? `; busy ${busy}` : ""}; ${checker}; ${machine}`;
}

/** A cache's state from a grid's cache.json: none, cold (no hit), warm (at least 95% hits) or mixed, per cache. */
export function cacheState(stats) {
  if (!stats) return "none";
  const one = (c) => (!c || c.hits + c.misses === 0 ? "none" : c.hits === 0 ? "cold" : c.hits / (c.hits + c.misses) >= 0.95 ? "warm" : "mixed");
  return `runtime ${one(stats.hits !== undefined ? stats : null)}, typecheck ${one(stats.typecheck)}`;
}
