// The throughput regression gate: two focus runs' perf.json (focus.mjs, perf.mjs) compared unit by unit (a grid, or a grid's code
// with --split-codes): programs per second, the checker's p50 and p99 wall time, and the programs over the cost budget. A metric more
// than `--threshold` (0.2) worse is flagged: a rate that fell below old/(1+t), a time above old*(1+t) and at least `--floor-ms` (20)
// slower, an over-budget count above old*(1+t) and at least `--floor-count` (2) higher. Only comparable runs are compared: the same
// units, the same programs and sample per unit, the same seed, checker mode (warm, role log), budget and core count, and the same
// cache state per unit (none, cold, warm or mixed, each run's stated). Anything else is "not comparable", never a pass.
//
//   node scripts/fuzz/perf-compare.mjs <old run dir> <new run dir> [--threshold 0.2] [--floor-ms 20] [--floor-count 2]
//
// Exit 0: comparable, nothing worse; 1: something worse; 2: not comparable (or no perf.json).
import { existsSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const DEFAULTS = Object.freeze({ threshold: 0.2, floorMs: 20, floorCount: 2 });

/** A run directory's perf.json, or a thrown error naming what is missing. */
export function readRun(dir) {
  const path = join(resolve(dir), "perf.json");
  if (!existsSync(path)) throw new Error(`no perf.json in ${dir} (a focus.mjs run writes it; SOLID_CHECKER_FUZZ_PERF=0 turns it off)`);
  return JSON.parse(readFileSync(path, "utf8"));
}

/** Why `older` and `newer` cannot be compared: a list, empty when they can. */
export function incomparable(older, newer) {
  const reasons = [];
  const differs = (what, a, b) => { if (JSON.stringify(a ?? null) !== JSON.stringify(b ?? null)) reasons.push(`${what}: ${JSON.stringify(a ?? null)} vs ${JSON.stringify(b ?? null)}`); };
  differs("codes", older.codes, newer.codes);
  differs("seed", older.seed, newer.seed);
  differs("selection", older.selection, newer.selection);
  differs("checker mode", older.mode, newer.mode);
  differs("budget", older.budget, newer.budget);
  differs("cores", older.host?.cpus, newer.host?.cpus);
  const byLabel = new Map(newer.units.map((unit) => [unit.label, unit]));
  differs("units", older.units.map((unit) => unit.label), newer.units.map((unit) => unit.label));
  for (const unit of older.units) {
    const other = byLabel.get(unit.label);
    if (!other) continue;
    differs(`${unit.label} programs`, unit.programs, other.programs);
    differs(`${unit.label} sample`, unit.gridSample, other.gridSample);
    differs(`${unit.label} cache`, unit.cache, other.cache);
  }
  return reasons;
}

const change = (a, b) => (typeof a === "number" && typeof b === "number" && a > 0 ? Math.round(((b - a) / a) * 1000) / 10 : null);

/** One unit's metrics compared: each { metric, old, new, changePercent, worse }. */
export function compareUnit(older, newer, { threshold, floorMs, floorCount } = DEFAULTS) {
  const rate = (a, b) => typeof a === "number" && typeof b === "number" && b > 0 && a / b - 1 > threshold;
  const time = (a, b) => typeof a === "number" && typeof b === "number" && b - a >= floorMs && b > a * (1 + threshold);
  const count = (a, b) => typeof a === "number" && typeof b === "number" && b - a >= floorCount && b > a * (1 + threshold);
  const metric = (name, a, b, worse) => ({ metric: name, old: a ?? null, new: b ?? null, changePercent: change(a, b), worse: worse(a, b) });
  return [
    metric("programs/s", older.programsPerSecond, newer.programsPerSecond, rate),
    metric("checker p50 ms", older.checker?.p50, newer.checker?.p50, time),
    metric("checker p99 ms", older.checker?.p99, newer.checker?.p99, time),
    metric("over budget", older.checker?.overBudget, newer.checker?.overBudget, count),
  ];
}

/** The comparison of two runs' perf.json: `{ comparable, reasons, cache, units: [{ label, metrics, worse }], worse }`. */
export function comparePerf(older, newer, options = {}) {
  const settings = { ...DEFAULTS, ...options };
  const reasons = incomparable(older, newer);
  const cache = { old: Object.fromEntries(older.units.map((unit) => [unit.label, unit.cache])), new: Object.fromEntries(newer.units.map((unit) => [unit.label, unit.cache])) };
  if (reasons.length) return { comparable: false, reasons, cache, settings, units: [], worse: [] };
  const byLabel = new Map(newer.units.map((unit) => [unit.label, unit]));
  const units = older.units.map((unit) => {
    const metrics = compareUnit(unit, byLabel.get(unit.label), settings);
    return { label: unit.label, metrics, worse: metrics.filter((m) => m.worse).map((m) => m.metric) };
  });
  return { comparable: true, reasons, cache, settings, units, worse: units.filter((unit) => unit.worse.length).map((unit) => `${unit.label}: ${unit.worse.join(", ")}`) };
}

/** The comparison for a person. */
export function compareLines(result) {
  const { settings } = result;
  const lines = [`perf-compare: threshold ${Math.round(settings.threshold * 100)}% (floors ${settings.floorMs} ms, ${settings.floorCount} programs)`];
  const states = [...new Set([...Object.values(result.cache.old), ...Object.values(result.cache.new)].map((state) => state ?? "unknown"))];
  lines.push(`perf-compare: cache ${states.length === 1 ? `${states[0]} in both runs` : `old ${JSON.stringify(result.cache.old)}, new ${JSON.stringify(result.cache.new)}`}`);
  if (!result.comparable) {
    lines.push("perf-compare: NOT COMPARABLE");
    for (const reason of result.reasons) lines.push(`  ${reason}`);
    return lines;
  }
  const show = (v) => (v === null || v === undefined ? "-" : String(v));
  for (const unit of result.units) {
    const parts = unit.metrics.map((m) => `${m.metric} ${show(m.old)} -> ${show(m.new)}${m.changePercent === null ? "" : ` (${m.changePercent > 0 ? "+" : ""}${m.changePercent}%)`}${m.worse ? " WORSE" : ""}`);
    lines.push(`  ${unit.label}: ${parts.join("; ")}`);
  }
  lines.push(result.worse.length ? `perf-compare: ${result.worse.length} unit(s) worse: ${result.worse.join("; ")}` : "perf-compare: nothing worse");
  return lines;
}

function main(argv) {
  const dirs = [];
  const options = {};
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    const key = { "--threshold": "threshold", "--floor-ms": "floorMs", "--floor-count": "floorCount" }[arg];
    if (key) {
      const value = Number(argv[++index]);
      if (!(value >= 0)) throw new Error(`${arg} takes a non-negative number`);
      options[key] = value;
    } else if (arg.startsWith("--")) throw new Error(`perf-compare.mjs: unknown option ${arg}`);
    else dirs.push(arg);
  }
  if (dirs.length !== 2) throw new Error("usage: node scripts/fuzz/perf-compare.mjs <old run dir> <new run dir> [--threshold 0.2] [--floor-ms 20] [--floor-count 2]");
  const result = comparePerf(readRun(dirs[0]), readRun(dirs[1]), options);
  for (const line of compareLines(result)) console.log(line);
  return result.comparable ? (result.worse.length ? 1 : 0) : 2;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try { process.exitCode = main(process.argv.slice(2)); } catch (error) { console.error(error.message); process.exitCode = 2; }
}
