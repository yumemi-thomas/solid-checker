// The sampler worker of perf.mjs `startSampler`: /proc/stat and /proc/diskstats every `intervalMs` into the shared ring.
import { readFileSync } from "node:fs";
import { isMainThread, workerData } from "node:worker_threads";
import { CAPACITY, FIELDS, parseDiskstats, parseProcStat } from "./perf.mjs";

/** The sampler worker's loop (`startSampler`). */
export async function samplerLoop({ head, ring, intervalMs, disks }) {
  const count = new Int32Array(head);
  const values = new Float64Array(ring);
  const names = new Set(disks);
  const sample = () => {
    const cpu = parseProcStat(readFileSync("/proc/stat", "utf8"));
    const sectors = parseDiskstats(readFileSync("/proc/diskstats", "utf8"), names);
    const n = Atomics.load(count, 0);
    values.set([Date.now(), cpu.busy, cpu.idle, cpu.iowait, sectors], (n % CAPACITY) * FIELDS);
    Atomics.store(count, 0, n + 1);
  };
  for (;;) {
    try { sample(); } catch { /* a missed sample leaves a longer interval, nothing else */ }
    await new Promise((resolve) => setTimeout(resolve, intervalMs));
  }
}

if (!isMainThread && workerData?.ring) await samplerLoop(workerData);
