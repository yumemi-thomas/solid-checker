// The fuzzer's short loop (`make fuzz-focus CODES=A,B`): the six grids (misuse and legal; base, sources, combinations) restricted to the
// given codes, on the Rust executor and the Chrome host, each with its oracle sample (one program per stratum in real Chrome through
// `feedback run`). It prints each grid's outcomes and the oracle comparison, lists what needs a look, and exits 1 when anything does:
// a false clean, a wrong rule, a false positive, a legal program the runtime contradicts, a harness failure, or a program whose record
// differs from the oracle's in behaviour. Tens of seconds per code, about half that again when the cache holds the programs. The full
// grids run before a pull request (scripts/fuzz/README.md).
//
//   node scripts/fuzz/focus.mjs --codes A,B --checker-bin <solid-checker-rust> --typefacts <solid-typefacts>
//     [--grids mx-base,mx-src,mx-combo,lg-base,lg-src,lg-combo] [--seed 1] [--out <dir>] [--runtime-diff <per stratum>]
//     [--oracle-axes code,context,outcome] [--smoke [--sample 0.015] [--sample-seed <n>]] [-- <grid args>]
//     [--split-codes] [--prune-cases] [--grid-timeout <minutes>] [--heap-mb <MiB>]
// `--split-codes`: one grid process per code (`<grid>/<code>/`), so a large grid (mx-recall, 277k programs) stays within a heap and a
// failure or hang costs one code; `--prune-cases`: delete each grid's `cases/` and `chrome-cases/` once its rows are read (a program is
// a directory of files: three full runs filled a VM disk's inodes); `--grid-timeout`: a grid process that runs longer is killed and
// reported (default 120 minutes; a stalled Chrome or a paused VM once hung a run for hours); `--heap-mb`: the grid process's heap
// (default 12000 for the recall grids, Node's default otherwise). Each grid's output streams to `<grid>.log` while it runs.
// `--smoke` (`make fuzz-smoke`): every code, each grid restricted to its smoke manifest (smoke.mjs, scripts/fuzz/smoke/<grid>.json),
// plus `--sample` of its other programs picked anew each run (`--sample-seed` repeats a run's pick; the summary prints it).
// The loop's oracle sample is stratified by code, context and outcome (every outcome of every context in real Chrome); the full run's
// by code, context, source, mount and outcome (runtime-diff.mjs DEFAULT_DIFF_AXES), many more programs. `--oracle-axes` picks.
import { spawnSync } from "node:child_process";
import { closeSync, existsSync, mkdirSync, openSync, readFileSync, rmSync, statfsSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { legalCells } from "./legal.mjs";
import { cellList } from "./matrix.mjs";
import { SMOKE_DIR, movedVerdicts, readRows } from "./smoke.mjs";
import { BEHAVIOUR_KINDS } from "./runtime-diff.mjs";
import { NODE } from "../lib/node-runtime.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const MOUNTS = ["--mounts", "root,child,show,for"];
const SOURCES = ["--sources", "props,context,module,async"];
/** The grids of the full run (the same arguments), by name. */
export const GRIDS = {
  "mx-base": ["matrix.mjs", ...MOUNTS],
  "mx-src": ["matrix.mjs", ...SOURCES],
  "mx-combo": ["matrix.mjs", "--combine", ...MOUNTS],
  "lg-base": ["legal.mjs", ...MOUNTS],
  "lg-src": ["legal.mjs", ...SOURCES],
  "lg-combo": ["legal.mjs", "--combine", ...MOUNTS],
  // The recall axes (matrix.mjs recallAxes): asked for by name (`--grids mx-recall,lg-recall`), never by default; their false cleans
  // are the measured known misses, not regressions.
  "mx-recall": ["matrix.mjs", "--recall"],
  "lg-recall": ["legal.mjs", "--recall"],
};
/** The grids a focus or smoke run takes when `--grids` is not given: every grid but the recall ones. */
export const DEFAULT_GRIDS = Object.keys(GRIDS).filter((grid) => !grid.endsWith("-recall"));
/** Outcomes that need a look: a misuse grid's and a legal grid's. */
export const PROBLEMS = ["false-clean", "wrong-rule", "harness", "false-positive", "not-legal"];

/** One grid's outcomes (programs.jsonl) and its oracle comparison (runtime-diff.json), from its output directory. */
export function readGrid(out) {
  const programs = join(out, "programs.jsonl");
  const outcomes = {};
  const flagged = [];
  if (existsSync(programs)) {
    for (const line of readFileSync(programs, "utf8").split("\n").filter(Boolean)) {
      const row = JSON.parse(line);
      outcomes[row.outcome] = (outcomes[row.outcome] ?? 0) + 1;
      if (PROBLEMS.includes(row.outcome)) flagged.push({ id: row.id, outcome: row.outcome, detail: row.detail });
    }
  }
  const diff = join(out, "runtime-diff.json");
  const oracle = existsSync(diff) ? JSON.parse(readFileSync(diff, "utf8")) : null;
  const behaviour = (oracle?.differing ?? []).filter((result) => result.diffs.some((d) => BEHAVIOUR_KINDS.includes(d.kind)));
  return { outcomes, flagged, oracle: oracle?.summary ?? null, behaviour };
}

/**
 * One grid process: Node with `heapMb` of heap when given, its stdout and stderr streamed to `logPath` while it runs (a long grid's
 * progress lines are readable before it ends), killed after `timeoutMs`. Returns `{ status, signal, timedOut }`.
 */
/** Stops a focus run: what follows would fail the same way (no room, no executor). */
export class FocusStop extends Error {}

/** Why `dir`'s disk cannot hold another grid (under 5 GiB or 5% of the inodes free), or null. */
export function roomAt(dir, { statfs = statfsSync } = {}) {
  let stats;
  try { stats = statfs(dir); } catch { return null; }
  const freeBytes = Number(stats.bavail) * Number(stats.bsize);
  if (freeBytes < 5 * 2 ** 30) return `${(freeBytes / 2 ** 30).toFixed(1)} GiB free`;
  if (Number(stats.files) > 0 && Number(stats.ffree) / Number(stats.files) < 0.05) return `${stats.ffree} of ${stats.files} inodes free`;
  return null;
}

export function runGridProcess(script, args, { logPath, timeoutMs, heapMb, runtime = NODE }) {
  mkdirSync(dirname(logPath), { recursive: true });
  const log = openSync(logPath, "w");
  try {
      const run = spawnSync(runtime, [...(heapMb ? [`--max-old-space-size=${heapMb}`] : []), script, ...args], { stdio: ["ignore", log, log], timeout: timeoutMs, killSignal: "SIGTERM" });
      return { status: run.status, signal: run.signal, timedOut: run.error?.code === "ETIMEDOUT" };
    } finally {
      closeSync(log);
    }
  }

  export function parse(argv) {
    const split = argv.indexOf("--");
    const own = split < 0 ? argv : argv.slice(0, split);
    const options = { grids: DEFAULT_GRIDS.join(","), seed: "1", runtimeDiff: "1", oracleAxes: "code,context,outcome", sample: "0.015", sampleSeed: String(Date.now() % 1000000007), extra: split < 0 ? [] : argv.slice(split + 1) };
    for (let index = 0; index < own.length; index += 2) {
      const key = own[index].replace(/^--/, "").replace(/-([a-z])/g, (_, c) => c.toUpperCase());
      if (key === "smoke") { options.smoke = true; options.codes ??= "all"; index -= 1; continue; }
      if (key === "splitCodes" || key === "pruneCases") { options[key] = true; index -= 1; continue; }
      if (!["codes", "grids", "seed", "out", "runtimeDiff", "oracleAxes", "sample", "sampleSeed", "checkerBin", "typefacts", "gridTimeout", "heapMb"].includes(key) || own[index + 1] === undefined) throw new Error(`focus.mjs: unknown or empty option ${own[index]}`);
      options[key] = own[index + 1];
    }
    if (!options.codes) throw new Error("focus.mjs: --codes A,B (the codes the change touches)");
    // A smoke manifest names one grid's programs across its codes: a per-code process would not generate them all.
    if (options.splitCodes && options.smoke) throw new Error("focus.mjs: --split-codes does not apply to --smoke (a smoke grid is small)");
    for (const grid of options.grids.split(",")) if (!GRIDS[grid]) throw new Error(`focus.mjs: no grid ${grid} (${Object.keys(GRIDS).join(", ")})`);
    return options;
  }

  function main() {
    const options = parse(process.argv.slice(2));
    const out = resolve(options.out ?? join(tmpdir(), "solid-checker-fuzz-focus.noindex", `${Date.now()}`));
    mkdirSync(out, { recursive: true });
    const tools = [...(options.checkerBin ? ["--checker-bin", options.checkerBin] : []), ...(options.typefacts ? ["--typefacts", options.typefacts] : [])];
    const oracle = options.runtimeDiff === "0" ? [] : ["--runtime-diff", options.runtimeDiff, "--runtime-diff-axes", options.oracleAxes];
    const host = ["--all-cores", "--fast-host", "chrome", "--chrome-pages", "reuse", "--chrome-clock", "virtual-time", "--executor", "rust", ...oracle];
    const attention = [];
    console.log(`fuzz-${options.smoke ? "smoke" : "focus"}: codes ${options.codes}; output ${out}${options.smoke ? `; sample ${options.sample} of the other programs, seed ${options.sampleSeed} (--sample-seed ${options.sampleSeed} repeats it)` : ""}`);
    // `--codes all`: every code (the full grids); `--runtime-diff 0`: no oracle sample.
    const codes = options.codes === "all" ? undefined : options.codes.split(",");
    // One grid process: its output streamed to `<dir>.log`, a deadline, then its rows judged.
    const runUnit = (grid, script, shape, unit, manifest) => {
      // Room for the grid before it starts: a run that fills the disk midway fails late, with ENOSPC rows (ADR 0519's preflight runs at
      // sync only; a run of many codes writes a directory per program).
      const room = roomAt(dirname(unit.dir));
      if (room) throw new FocusStop(`${unit.label}: not started, ${room} (prune cases/ with --prune-cases or free the disk)`);
      const started = Date.now();
      const select = [...(unit.codes ? ["--codes", unit.codes.join(",")] : []), ...(options.smoke ? ["--ids", manifest, "--ids-sample", options.sample, "--ids-sample-seed", options.sampleSeed] : [])];
      const heap = options.heapMb ?? (grid.endsWith("-recall") ? "12000" : undefined);
      const logPath = `${unit.dir}.log`;
      const run = runGridProcess(join(here, script), ["--seed", options.seed, "--grid", ...shape, ...select, "--out", unit.dir, ...tools, ...host, ...options.extra], { logPath, timeoutMs: Number(options.gridTimeout ?? 120) * 60_000, heapMb: heap });
      const seconds = Math.round((Date.now() - started) / 1000);
      const { outcomes, flagged, oracle, behaviour } = readGrid(unit.dir);
      const total = Object.values(outcomes).reduce((a, b) => a + b, 0);
      const summary = oracle ? `oracle ${oracle.compared} compared, ${oracle.behaviour} behaviour, ${oracle.attributionOnly} attribution, ${oracle.inconclusive ?? 0} inconclusive` : "no oracle sample";
      console.log(`  ${unit.label.padEnd(8)} ${String(seconds).padStart(4)}s  ${total} programs ${JSON.stringify(outcomes)}  ${summary}`);
      const tail = () => readFileSync(logPath, "utf8").trim().split("\n").slice(-3).join(" | ").slice(0, 400);
      if (run.timedOut) attention.push(`${unit.label}: killed after ${options.gridTimeout ?? 120} minutes (${logPath}): ${tail()}`);
      else if (run.status !== 0) attention.push(`${unit.label}: exited ${run.status ?? run.signal} (${logPath}): ${tail()}`);
      // A run that judged nothing is no clean grid (a full disk, a crashed host).
      if (total === 0) attention.push(`${unit.label}: no program was judged (${logPath})`);
      // An executor that cannot start fails every grid the same way: the rest of the run would only repeat it.
      if (total === 0 && /the executor did not start/.test(readFileSync(logPath, "utf8"))) throw new FocusStop(`${unit.label}: the executor did not start (${tail()})`);
      for (const row of flagged.slice(0, 10)) attention.push(`${unit.label}: ${row.outcome} ${row.id} ${JSON.stringify(row.detail ?? {}).slice(0, 200)}`);
      if (flagged.length > 10) attention.push(`${unit.label}: ... ${flagged.length - 10} more (${join(unit.dir, "programs.jsonl")})`);
      for (const result of behaviour.slice(0, 10)) attention.push(`${unit.label}: oracle differs ${result.id} ${result.diffs.map((d) => `${d.kind} ${d.detail}`).join("; ").slice(0, 200)}`);
      // A smoke run: every program whose behaviour moved from the manifest's; a move to a worse outcome needs a look.
      if (options.smoke && existsSync(join(unit.dir, "programs.jsonl"))) {
        const moved = movedVerdicts(JSON.parse(readFileSync(manifest, "utf8")), readRows(unit.dir));
        if (moved.worse.length || moved.other.length) console.log(`           moved from the manifest: ${moved.worse.length} worse, ${moved.other.length} better or changed`);
        if (moved.missing.length) attention.push(`${unit.label}: ${moved.missing.length} programs of the manifest gave no row (${moved.missing.slice(0, 3).join(", ")})`);
        for (const move of moved.worse.slice(0, 10)) attention.push(`${unit.label}: worse ${move.id} ${move.from} -> ${move.to}`);
        if (moved.worse.length > 10) attention.push(`${unit.label}: ... ${moved.worse.length - 10} more worse`);
        for (const move of moved.other.slice(0, 5)) console.log(`             ${move.id} ${move.from} -> ${move.to}`);
      }
      if (oracle?.inconclusive) attention.push(`${unit.label}: ${oracle.inconclusive} oracle runs inconclusive (${join(unit.dir, "runtime-diff.jsonl")})`);
      if (options.pruneCases) for (const name of ["cases", "chrome-cases"]) rmSync(join(unit.dir, name), { recursive: true, force: true });
    };
    try {
    for (const grid of options.grids.split(",")) {
      const [script, ...shape] = GRIDS[grid];
      // A grid with no cell for these codes (a code with no legal context, say) has nothing to run.
      const cells = script === "legal.mjs" ? legalCells({ codes, recall: shape.includes("--recall") }) : cellList({ codes, recall: shape.includes("--recall") });
      if (!cells.length) { console.log(`  ${grid.padEnd(8)}    -  no cells for these codes`); continue; }
      const manifest = join(SMOKE_DIR, `${grid}.json`);
      if (options.smoke && !existsSync(manifest)) { attention.push(`${grid}: no smoke manifest ${manifest} (smoke.mjs select)`); continue; }
      // `--split-codes`: one process per code of the grid's cells, in the cells' order.
      const units = options.splitCodes ? [...new Set(cells.map((cell) => cell.code))].map((code) => ({ codes: [code], dir: join(out, grid, code), label: `${grid}/${code}` })) : [{ codes, dir: join(out, grid), label: grid }];
      for (const unit of units) runUnit(grid, script, shape, unit, manifest);
    }
  } catch (error) {
    if (!(error instanceof FocusStop)) throw error;
    attention.push(`stopped: ${error.message}`);
    process.exitCode = 2;
  }
  if (attention.length) {
    console.log(`\nneeds a look (${attention.length}):`);
    for (const line of attention) console.log(`  ${line}`);
    process.exitCode ||= 1;
  } else console.log("\nnothing needs a look");
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main();
