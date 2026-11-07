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
//     [--split-codes] [--prune-cases] [--grid-timeout <minutes>] [--heap-mb <MiB>] [--role-log [--roles-against <roles.json>]] [--recall-full]
//     [--scratch <dir>] [--sample-big <fraction> [--sample-codes A,B] [--sample-big-seed N] [--sample-big-min K]] [--warm-checker]
//     [--budget-ms 1000] [--budget-mb 500] [--idle-stretch-s 10] [--perf-against <earlier run dir> [--perf-threshold 0.2]]
// `--split-codes`: one grid process per code (`<grid>/<code>/`), so a large grid (mx-recall, 277k programs) stays within a heap and a
// failure or hang costs one code; `--prune-cases`: delete each grid's `cases/` and `chrome-cases/` once its rows are read (a program is
// a directory of files: three full runs filled a VM disk's inodes); `--grid-timeout`: a grid process that runs longer is killed and
// reported (default 120 minutes; a stalled Chrome or a paused VM once hung a run for hours); `--heap-mb`: the grid process's heap
// (default 12000 for the recall grids, Node's default otherwise). Each grid's output streams to `<grid>.log` while it runs.
// `--scratch` (default $SOLID_CHECKER_FUZZ_SCRATCH, which vm.sh points at the VM's tmpfs): each grid's `cases/` and `chrome-cases/`
// live there behind links at their usual paths (scratch.mjs), so the per-program writes never reach a disk. The role log does not: it
// is one file that grows to gigabytes (7.5 GB on a full lg-recall, which filled the VM's 16 GB tmpfs and got a 103k-program run
// OOM-killed), so it is written beside the grid output, on the data disk.
// Before every unit the free space of the grid output (where the role log goes) and of the scratch is checked (`roomAt`): a unit that
// would not fit stops the run with a "needs a look" reason, never as `harness` rows of ENOSPC; a unit whose log shows the disk filled
// stops it too.
// `--role-log` (ADR 0533): each grid's checkers append the role log to `<grid>.roles.jsonl` (one file: a directory would take an inode
// per program); it is folded into `<grid>/roles.json` and the run's
// `<out>/roles.json` (roles.mjs: arms hit and never hit, yield, new arms against `--roles-against`, silent-by-default decisions).
// `--warm-checker` (ADR 0543): each grid's slot checks go to one retained checker daemon per executor slot; with `--role-log` each
// check names its program, which the daemon's log lines carry.
// `--smoke` (`make fuzz-smoke`): every code, each grid restricted to its smoke manifest (smoke.mjs, scripts/fuzz/smoke/<grid>.json),
// plus `--sample` of its other programs picked anew each run (`--sample-seed` repeats a run's pick; the summary prints it).
// `--sample-big <fraction>` (sample.mjs): while iterating, the two codes that are ~40% of a full run (STORE_WRITE_OUTSIDE_SETTER,
// REACTIVE_WRITE_IN_OWNED_SCOPE; `--sample-codes` names others) keep that fraction of every (code, context, layer, variant) stratum,
// at least `--sample-big-min` (1) programs each, picked by `--sample-big-seed` (1); every other code stays whole, each grid records
// the sample (`gridSample` in matrix.json and legal.json), the summary says so, and the matrix gate refuses a sampled matrix. A run
// without the flag (the full run before a PR) is complete.
// The run's self-profile (perf.mjs, cost.mjs; measurement only, no row or verdict reads it): each grid writes its program costs
// (`costs.jsonl`, `cost.json`: checker wall time, CPU, peak RSS, role decisions with --role-log, every program over `--budget-ms` or
// `--budget-mb`) and its phases (`profile.json`); this process samples the machine's CPU and disks (Linux /proc) and prints one
// `perf <unit>:` line per unit (a grid, or a grid's code with --split-codes): programs/s, phases, checker p50/p99, CPU busy, idle
// stretches of at least `--idle-stretch-s`, disk MiB per program. It writes them to `<unit>/perf.json` and the run's to
// `<out>/perf.json`. `--perf-against <run dir>`: the run's perf.json against that run's (perf-compare.mjs); a comparable unit more than
// `--perf-threshold` worse, or runs that are not comparable, need a look. SOLID_CHECKER_FUZZ_PERF=0 records none of it.
// The loop's oracle sample is stratified by code, context and outcome (every outcome of every context in real Chrome); the full run's
// by code, context, source, mount and outcome (runtime-diff.mjs DEFAULT_DIFF_AXES), many more programs. `--oracle-axes` picks.
import { spawnSync } from "node:child_process";
import { Worker } from "node:worker_threads";
import { closeSync, existsSync, mkdirSync, openSync, readFileSync, rmSync, statfsSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve, basename } from "node:path";
import { fileURLToPath } from "node:url";
import { rowProblem } from "./codes.mjs";
import { gridStanding, heldLine, ledgerCovers, manifestIds, offLedger, readOpenDefects } from "./open-defects.mjs";
import { GRIDS } from "./focus-grids.mjs";
import { legalCells, pairLegalCells } from "./legal.mjs";
import { cellList, censusCells } from "./matrix.mjs";
import { FIXED_SEED, writeSample } from "./cover.mjs";
import { SMOKE_DIR, movedVerdicts, readRows } from "./smoke.mjs";
import { BEHAVIOUR_KINDS } from "./runtime-diff.mjs";
import { mergeGridReports, summaryLine } from "./roles.mjs";
import { PARTIAL, readPartial, stopReason } from "./partial.mjs";
import { BIG_CODES, sampleLine } from "./sample.mjs";
import { NODE } from "../lib/node-runtime.mjs";
import { cpus, platform, totalmem } from "node:os";
import { cacheState, machineWindow, perfLine, samplerUnavailable, startSampler, unitPerf } from "./perf.mjs";
import { compareLines, comparePerf, readRun } from "./perf-compare.mjs";

const here = dirname(fileURLToPath(import.meta.url));
export { GRIDS } from "./focus-grids.mjs";
/** The programs a sample sends through Chrome, per grid: the oracle runs about 2.2 programs a second. */
const ORACLE_CAP = 25;
/** The census's oracle strata. */
const CENSUS_ORACLE_AXES = "code,outcome";
/** The grid that takes no manifest: it holds every program it has, and answers to nothing recorded, so a run lists all of it. */
export const whole = (grid) => grid === "mx-census";
/** The grids a focus or smoke run takes when `--grids` is not given: every grid but the recall and pair ones. */
export const DEFAULT_GRIDS = Object.keys(GRIDS).filter((grid) => !grid.endsWith("-recall") && !grid.endsWith("-pairs"));
export { PROBLEMS } from "./codes.mjs";

/**
 * One grid's outcomes (programs.jsonl) and its oracle comparison (runtime-diff.json), from its output directory. A grid process that
 * stopped before writing programs.jsonl gives the rows it judged before (partial.mjs) and `partial: true`; `partial: false` when it
 * left neither.
 */
export function readGrid(out, { sampleSeed = null } = {}) {
  const programs = join(out, "programs.jsonl");
  const outcomes = {};
  const flagged = [];
  const complete = existsSync(programs);
  const rows = complete ? readFileSync(programs, "utf8").split("\n").filter(Boolean).map((line) => JSON.parse(line)) : readPartial(out);
  const list = readOpenDefects();
  const held = [];
  const unlisted = [];
  const strict = ledgerCovers(list, sampleSeed);
  const manifest = manifestIds(basename(out));
  for (const row of rows ?? []) {
    outcomes[row.outcome] = (outcomes[row.outcome] ?? 0) + 1;
    const problem = rowProblem(row);
    const standing = gridStanding(list, row, problem, { grid: basename(out), strict: strict || manifest.has(row.id) });
    if (standing?.kind === "held") held.push(standing.entry);
    else if (standing?.kind === "unlisted") unlisted.push(row.id);
    else if (standing?.kind === "fixed") flagged.push({ id: row.id, outcome: "open-defect-fixed", detail: { item: standing.entry.item, move: "from open-defects.json to rediscovery.json" } });
    else if (standing?.kind === "moved" || standing?.kind === "certified") flagged.push({ id: row.id, outcome: `open-defect-${standing.kind}`, detail: { listed: standing.listed, row: row.detail } });
    else if (problem) flagged.push({ id: row.id, outcome: problem, detail: row.detail });
  }
  if (held.length) console.log(`  ${heldLine(basename(out), held)}`);
  if (unlisted.length) console.log(`  ${basename(out)}: ${unlisted.length} failing rows outside the smoke manifest and off the ledger, not judged: sample seed ${sampleSeed}, ledger seed ${list.rows?.seed}`);
  const diff = join(out, "runtime-diff.json");
  const oracle = existsSync(diff) ? JSON.parse(readFileSync(diff, "utf8")) : null;
  const behaviour = (oracle?.differing ?? []).filter((result) => result.diffs.some((d) => BEHAVIOUR_KINDS.includes(d.kind)));
  // A sampled grid (`--sample-big`) and a shard (`--shard`) say so in their report, and so does the number of programs the grid
  // selected (null in a report from before it was recorded, or with no report).
  const path = ["matrix.json", "legal.json"].map((name) => join(out, name)).find((candidate) => existsSync(candidate));
  const report = path ? JSON.parse(readFileSync(path, "utf8")) : {};
  return { outcomes, flagged, oracle: oracle?.summary ?? null, behaviour, gridSample: report.gridSample ?? null, gridShard: report.gridShard ?? null, selected: report.selected ?? null, ...(complete ? {} : { partial: rows !== null }) };
}

/**
 * A run that judged nothing is no clean grid (a full disk, a crashed host), unless the grid finished and its report says it selected
 * no program: a code with no shape on the grid's axes (no source shape on mx-src, say), or a shard that got none. `--split-codes`
 * still flags a code that no grid had a program for.
 */
export function emptyByDesign({ run, grid, total }) {
  return total === 0 && grid.selected === 0 && grid.partial === undefined && run.status === 0 && !run.timedOut;
}

/**
 * What a finished grid process needs a look for, beyond its rows: a deadline it missed, an exit it failed, rows it never wrote (a
 * harness failure, with the reason its log gives and the rows it judged before it stopped), a run that judged nothing.
 */
export function unitProblems(label, { run, grid, total, log, logPath, timeoutMinutes }) {
  const problems = [];
  const tail = () => log.trim().split("\n").slice(-3).join(" | ").slice(0, 400);
  if (run.timedOut) problems.push(`${label}: killed after ${timeoutMinutes} minutes (${logPath}): ${tail()}`);
  else if (run.status !== 0) problems.push(`${label}: exited ${run.status ?? run.signal} (${logPath}): ${tail()}`);
  // No programs.jsonl: the grid process stopped (a crashed host, a killed executor, a refusal), whatever its exit said.
  if (grid.partial !== undefined) problems.push(`${label}: harness: the grid process stopped before writing its rows (${grid.partial ? `${total} judged before it stopped, kept in ${PARTIAL}` : "none judged"}): ${stopReason(log)}`);
  else if (total === 0 && !emptyByDesign({ run, grid, total })) problems.push(`${label}: no program was judged (${logPath})`);
  return problems;
}

/**
 * One grid process: Node with `heapMb` of heap when given, its stdout and stderr streamed to `logPath` while it runs (a long grid's
 * progress lines are readable before it ends), killed after `timeoutMs`. Returns `{ status, signal, timedOut }`.
 */
/** Stops a focus run: what follows would fail the same way (no room, no executor). */
export class FocusStop extends Error {}

/**
 * Why `dir`'s disk cannot hold another grid (under 5 GiB, `SOLID_CHECKER_FUZZ_MIN_FREE_GIB`, or 5% of the inodes free), or null. A `dir` that does not exist yet (a
 * grid's output before its first unit, the scratch before the first link) is measured at its nearest existing ancestor, which is the
 * disk it will be created on; one with no existing ancestor is not measured (null).
 */
export function roomAt(dir, { statfs = statfsSync, minGiB = Number(process.env.SOLID_CHECKER_FUZZ_MIN_FREE_GIB) || 5 } = {}) {
  let stats;
  for (let at = resolve(dir); ; at = dirname(at)) {
    try { stats = statfs(at); break; } catch { if (dirname(at) === at) return null; }
  }
  const freeBytes = Number(stats.bavail) * Number(stats.bsize);
  if (freeBytes < minGiB * 2 ** 30) return `${(freeBytes / 2 ** 30).toFixed(1)} GiB free`;
  if (Number(stats.files) > 0 && Number(stats.ffree) / Number(stats.files) < 0.05) return `${stats.ffree} of ${stats.files} inodes free`;
  return null;
}

/** A grid's role log: beside the grid directory, on the output's disk, never in the scratch (which is the VM's tmpfs). */
export function roleLogPath(unitDir) {
  return `${unitDir}.roles.jsonl`;
}

/**
 * The places a unit writes that can run out: the grid output (programs, logs, the role log) and, when there is one, the scratch
 * (`cases/`, `chrome-cases/`). Each is `[what, path]`.
 */
export function unitPlaces({ unitDir, scratch, roleLog }) {
  return [[roleLog ? "grid output and role log" : "grid output", dirname(unitDir)], ...(scratch ? [["scratch", scratch]] : [])];
}

/** The first of `places` (`unitPlaces`) without room, as a sentence, or null when every one has it. */
export function noRoom(places, options = {}) {
  for (const [what, path] of places) {
    const why = roomAt(path, options);
    if (why) return `${what} (${path}): ${why}`;
  }
  return null;
}

const FULL_DISK = /ENOSPC|no space left on device/i;

/** Whether a unit's log or flagged rows say the disk filled while it ran (what its `harness` rows would otherwise be read as). */
export function diskFilled({ log, flagged = [] }) {
  return FULL_DISK.test(log) || flagged.some((row) => FULL_DISK.test(JSON.stringify(row.detail ?? {})));
}

export function runGridProcess(script, args, { logPath, timeoutMs, heapMb, runtime = NODE, env = process.env }) {
  mkdirSync(dirname(logPath), { recursive: true });
  const log = openSync(logPath, "w");
  try {
      const run = spawnSync(runtime, [...(heapMb ? [`--max-old-space-size=${heapMb}`] : []), script, ...args], { stdio: ["ignore", log, log], timeout: timeoutMs, killSignal: "SIGTERM", env });
      return { status: run.status, signal: run.signal, timedOut: run.error?.code === "ETIMEDOUT" };
    } finally {
      closeSync(log);
    }
  }

  /**
   * The run's perf.json (perf-compare.mjs reads it): what makes two runs comparable (codes, seed, selection, checker mode, budget,
   * host), each unit's record in run order, and the machine over the whole run, its idle stretches named by the unit they fell in.
   */
  export function runPerf({ options, out, units, sampler, runStarted, runEnded, stretchOptions }) {
    const ordered = [...units].sort((a, b) => a.startedAt - b.startedAt);
    const machine = sampler ? machineWindow(sampler.samples(runStarted, runEnded), stretchOptions) : { why: samplerUnavailable() ?? "not sampled" };
    for (const stretch of machine.idleStretches ?? []) {
      const at = runStarted + stretch.atS * 1000;
      const unit = ordered.find((u) => at >= u.startedAt && at < u.endedAt);
      stretch.during = unit ? `during ${unit.label}` : ordered.some((u) => u.startedAt > at) ? "between units" : "after the last unit (folds, merge)";
    }
    const programs = ordered.reduce((a, u) => a + u.programs, 0);
    return {
      version: 1, out,
      codes: options.codes, seed: options.seed, ...(options.smoke ? { sampleSeed: options.sampleSeed } : {}),
      selection: { grids: options.grids, ...(!options.recallFull && options.grids.split(",").some((grid) => grid.endsWith("-recall")) ? { recall: "lean" } : {}), splitCodes: Boolean(options.splitCodes), smoke: Boolean(options.smoke), ...(options.smoke ? { sample: options.sample, sampleSeed: options.sampleSeed } : {}), sampleBig: options.sampleBig ?? null, ...(options.sampleBig ? { sampleCodes: options.sampleCodes ?? null, sampleBigSeed: options.sampleBigSeed ?? "1", sampleBigMin: options.sampleBigMin ?? "1" } : {}), oracle: options.runtimeDiff === "0" ? null : { perStratum: options.runtimeDiff, axes: options.oracleAxes }, extra: options.extra },
      mode: { warmChecker: Boolean(options.warmChecker), roleLog: Boolean(options.roleLog) },
      budget: { ms: Number(options.budgetMs ?? 1000), mb: Number(options.budgetMb ?? 500) },
      host: { cpus: cpus().length, memGiB: Math.round(totalmem() / 2 ** 30), platform: platform(), checkerBin: options.checkerBin ?? null },
      wallMs: runEnded - runStarted, programs, programsPerSecond: runEnded > runStarted ? Math.round((programs / ((runEnded - runStarted) / 1000)) * 100) / 100 : null,
      machine, units: ordered,
    };
  }

  /** The run's perf record as one line. */
  export function runPerfLine(report) {
    const m = report.machine;
    const machine = m.cpuBusy != null ? `cpu busy ${Math.round(m.cpuBusy * 100)}% (iowait ${Math.round(m.iowait * 100)}%), disk ${m.diskMiB} MiB, ${m.idleStretches.length} idle stretch${m.idleStretches.length === 1 ? "" : "es"}` : `machine not sampled (${m.why})`;
    return `perf: ${report.programs} programs in ${Math.round(report.wallMs / 1000)}s = ${report.programsPerSecond ?? "-"}/s over ${report.units.length} units; ${machine}`;
  }

  /** The grids' host: every core, the Chrome fast host on the Rust executor (audit.mjs runs its oracle grids on the same). */
  /** The most any one pool takes by default. A run shares the machine: every core is `--all-cores`, by name, on a machine that is free. */
  export const HOST_LIMIT = 6;
  export const hostLimits = (options = {}, cores = cpus().length) => {
    if (options.allCores) return ["--all-cores"];
    const limit = String(Math.min(HOST_LIMIT, cores));
    return ["--concurrency", limit, "--workers", limit, "--chrome-concurrency", limit];
  };
  export const hostArgs = (options = {}) => [...hostLimits(options), "--fast-host", "chrome", "--chrome-pages", "reuse", "--chrome-clock", "virtual-time", "--executor", "rust", ...(options.noWarmChecker ? ["--no-warm-checker"] : options.warmChecker ? ["--warm-checker"] : [])];

  export function parse(argv) {
    const split = argv.indexOf("--");
    const own = split < 0 ? argv : argv.slice(0, split);
    const options = { grids: DEFAULT_GRIDS.join(","), seed: "1", runtimeDiff: "1", oracleAxes: "code,context,source,mount,outcome", sample: "0.015", sampleSeed: String(Date.now() % 1000000007), extra: split < 0 ? [] : argv.slice(split + 1) };
    for (let index = 0; index < own.length; index += 2) {
      const key = own[index].replace(/^--/, "").replace(/-([a-z])/g, (_, c) => c.toUpperCase());
      if (key === "smoke") { options.smoke = true; options.codes ??= "all"; index -= 1; continue; }
      if (key === "allCores" || key === "splitCodes" || key === "pruneCases" || key === "roleLog" || key === "recallFull" || key === "recallLean" || key === "warmChecker" || key === "noWarmChecker") { options[key] = true; index -= 1; continue; }
      if (!["cover", "coverSeed", "oracleCap", "codes", "grids", "seed", "out", "runtimeDiff", "oracleAxes", "sample", "sampleSeed", "checkerBin", "typefacts", "gridTimeout", "heapMb", "rolesAgainst", "scratch", "sampleBig", "sampleCodes", "sampleBigSeed", "sampleBigMin", "budgetMs", "budgetMb", "idleStretchS", "perfAgainst", "perfThreshold", "shard"].includes(key) || own[index + 1] === undefined) throw new Error(`focus.mjs: unknown or empty option ${own[index]}`);
      options[key] = own[index + 1];
    }
    if (!options.codes) throw new Error("focus.mjs: --codes A,B (the codes the change touches)");
    options.scratch ??= process.env.SOLID_CHECKER_FUZZ_SCRATCH || undefined;
    if (options.rolesAgainst && !options.roleLog) throw new Error("focus.mjs: --roles-against needs --role-log");
    // A smoke manifest names one grid's programs across its codes: a per-code process would not generate them all.
    if (options.splitCodes && options.smoke) throw new Error("focus.mjs: --split-codes does not apply to --smoke (a smoke grid is small)");
    if (options.sampleBig !== undefined) {
      if (options.smoke) throw new Error("focus.mjs: --sample-big does not apply to --smoke (a smoke manifest names its programs)");
      if (!(Number(options.sampleBig) > 0 && Number(options.sampleBig) <= 1)) throw new Error(`focus.mjs: --sample-big takes a fraction in (0, 1], not ${options.sampleBig}`);
    } else if (options.sampleCodes || options.sampleBigSeed || options.sampleBigMin) throw new Error("focus.mjs: --sample-codes, --sample-big-seed and --sample-big-min need --sample-big");
    if (options.perfThreshold && !options.perfAgainst) throw new Error("focus.mjs: --perf-threshold needs --perf-against");
    if (options.shard !== undefined) {
      if (options.smoke) throw new Error("focus.mjs: --shard does not apply to --smoke (a smoke manifest names its programs)");
      const [index, count] = String(options.shard).split("/").map(Number);
      if (!/^\d+\/\d+$/.test(options.shard) || !(count >= 1) || !(index < count)) throw new Error(`focus.mjs: --shard takes i/N with 0 <= i < N, not ${options.shard}`);
    }
    for (const key of ["budgetMs", "budgetMb", "idleStretchS", "perfThreshold"]) if (options[key] !== undefined && !(Number(options[key]) >= 0)) throw new Error(`focus.mjs: --${key.replace(/[A-Z]/g, (c) => `-${c.toLowerCase()}`)} takes a non-negative number`);
    // A smoke run is the census and a covering sample of every grid (ADR 0615), the behaviour manifests and the ledger's programs within it.
    if (options.smoke) {
      options.cover ??= "all";
      options.oracleCap ??= String(ORACLE_CAP);
      if (options.grids === DEFAULT_GRIDS.join(",")) options.grids = Object.keys(GRIDS).join(",");
    }
    if (options.cover && !["all", "per-code"].includes(options.cover)) throw new Error("focus.mjs: --cover takes all or per-code");
    if (options.cover && options.codes !== "all") throw new Error("focus.mjs: --cover samples every code: it takes --codes all");
    for (const grid of options.grids.split(",")) if (!GRIDS[grid]) throw new Error(`focus.mjs: no grid ${grid} (${Object.keys(GRIDS).join(", ")})`);
    return options;
  }

  async function main() {
    const options = parse(process.argv.slice(2));
    const out = resolve(options.out ?? join(tmpdir(), "solid-checker-fuzz-focus.noindex", `${Date.now()}`));
    mkdirSync(out, { recursive: true });
    const tools = [...(options.checkerBin ? ["--checker-bin", options.checkerBin] : []), ...(options.typefacts ? ["--typefacts", options.typefacts] : [])];
    const oracle = options.runtimeDiff === "0" ? [] : ["--runtime-diff", options.runtimeDiff, "--runtime-diff-axes", options.oracleAxes];
    const host = [...hostArgs(options), ...oracle, ...(options.oracleCap ? ["--runtime-diff-cap", options.oracleCap] : [])];
    // `--sample-big`: the grids' stratified sample of the big codes (sample.mjs); the others stay whole.
    const sampleCodes = options.sampleCodes ? options.sampleCodes.split(",") : BIG_CODES;
    const sampled = options.sampleBig === undefined ? [] : ["--grid-sample", options.sampleBig, "--grid-sample-codes", sampleCodes.join(","), "--grid-sample-seed", options.sampleBigSeed ?? "1", "--grid-sample-min", options.sampleBigMin ?? "1"];
    // `--shard i/N`: every grid keeps its part i of N (shard.mjs); the N parts of a run are the whole run.
    const shardArgs = options.shard === undefined ? [] : ["--grid-shard", options.shard];
    const budgetArgs = [...(options.budgetMs ? ["--budget-ms", options.budgetMs] : []), ...(options.budgetMb ? ["--budget-mb", options.budgetMb] : [])];
    // The self-profile: the machine sampler (null off Linux or with SOLID_CHECKER_FUZZ_PERF=0) and each unit's record.
    const perfOn = process.env.SOLID_CHECKER_FUZZ_PERF !== "0";
    const sampler = perfOn ? startSampler() : null;
    const stretchOptions = { minSeconds: Number(options.idleStretchS ?? 10) };
    const runStarted = Date.now();
    const perfUnits = [];
    const attention = [];
    const scratchRoot = options.scratch ? join(resolve(options.scratch), `focus-${process.pid}-${Date.now()}`) : null;
    const folded = [];
    const pending = [];
    const sampledUnits = [];
    // Programs judged per code (with `--split-codes`) and in the whole run: a code no grid had a program for needs a look.
    const judgedByCode = new Map();
    let runTotal = 0;
    const against = options.rolesAgainst ? { ...JSON.parse(readFileSync(resolve(options.rolesAgainst), "utf8")), path: resolve(options.rolesAgainst) } : null;
    console.log(`fuzz-${options.smoke ? "smoke" : "focus"}: codes ${options.codes}; output ${out}${options.smoke ? `; sample ${options.sample} of the other programs, seed ${options.sampleSeed} (--sample-seed ${options.sampleSeed} repeats it)` : ""}${sampled.length ? `; SAMPLED: ${options.sampleBig} of every stratum of ${sampleCodes.join(", ")} (seed ${options.sampleBigSeed ?? "1"}, at least ${options.sampleBigMin ?? "1"} each), not a full run` : ""}${options.shard === undefined ? "" : `; SHARD ${options.shard} of every grid, one part of the run`}`);
    // `--codes all`: every code (the full grids); `--runtime-diff 0`: no oracle sample.
    const codes = options.codes === "all" ? undefined : options.codes.split(",");
    // One grid process: its output streamed to `<dir>.log`, a deadline, then its rows judged.
    const covered = options.cover ? writeSample({ perCode: options.cover === "per-code", seed: Number(options.coverSeed ?? FIXED_SEED), grids: options.grids.split(",").filter((grid) => !whole(grid)), runSeed: options.seed, dir: join(out, "cover") }) : null;
    const runUnit = (grid, script, shape, unit, manifest) => {
      // Room for the grid before it starts: a run that fills the disk midway fails late, with ENOSPC rows (ADR 0519's preflight runs at
      // sync only; a run of many codes writes a directory per program).
      // The role log is on that disk, and the scratch (a tmpfs on the VM) holds the cases: both are checked.
      const unitScratch = scratchRoot ? join(scratchRoot, unit.label) : null;
      const room = noRoom(unitPlaces({ unitDir: unit.dir, scratch: scratchRoot, roleLog: options.roleLog }));
      if (room) throw new FocusStop(`${unit.label}: not started, ${room} (prune cases/ with --prune-cases or free the disk)`);
      const started = Date.now();
      const select = [...(unit.codes ? ["--codes", unit.codes.join(",")] : []), ...(covered?.[grid] ? ["--ids", covered[grid].path, "--ids-sample", options.sample, "--ids-sample-seed", options.sampleSeed] : [])];
      // A recall grid is lean unless `--recall-full` (matrix.mjs LEAN_DEMOTED); the flag is the recall grids' only, the others take none.
      const recallArgs = options.recallFull && shape.includes("--recall") ? ["--recall-full"] : [];
      // A census cell is a pair: the default strata would send most of the grid through Chrome, ten minutes against seconds of evaluation.
      const unitHost = whole(grid) && options.runtimeDiff !== "0" ? host.map((arg) => (arg === options.oracleAxes ? CENSUS_ORACLE_AXES : arg)) : host;
      const heap = options.heapMb ?? (grid.endsWith("-recall") ? "12000" : undefined);
      const logPath = `${unit.dir}.log`;
      // The role log: one file per grid that every checker process appends whole lines to (role_log.rs), folded once the grid's rows
      // are written; a line two processes interleaved is counted as malformed, never folded.
      // On the output's disk even with a scratch directory: it grows to gigabytes and would fill the scratch's tmpfs.
      const roles = options.roleLog ? roleLogPath(unit.dir) : null;
      if (roles) { mkdirSync(dirname(roles), { recursive: true }); rmSync(roles, { force: true }); }
      const env = roles ? { ...process.env, SOLID_CHECKER_IR_ROLE_LOG: roles } : process.env;
      // A unit with none of the sampled codes runs whole without the flag (its report then says nothing of a sample).
      const unitSampled = sampled.length && (!unit.codes || unit.codes.some((code) => sampleCodes.includes(code))) ? sampled : [];
      const run = runGridProcess(join(here, script), ["--seed", options.seed, "--grid", ...shape, ...select, ...unitSampled, ...shardArgs, ...budgetArgs, ...recallArgs, "--out", unit.dir, ...(unitScratch ? ["--scratch", unitScratch] : []), ...tools, ...unitHost, ...options.extra], { logPath, timeoutMs: Number(options.gridTimeout ?? 120) * 60_000, heapMb: heap, env });
      const ended = Date.now();
      const seconds = Math.round((ended - started) / 1000);
      const read = readGrid(unit.dir, { sampleSeed: options.smoke ? options.sampleSeed : null });
      const { outcomes, flagged, oracle, behaviour, gridSample, gridShard } = read;
      const total = Object.values(outcomes).reduce((a, b) => a + b, 0);
      const summary = oracle ? `oracle ${oracle.compared} compared, ${oracle.behaviour} behaviour, ${oracle.attributionOnly} attribution, ${oracle.inconclusive ?? 0} inconclusive` : "no oracle sample";
      const counted = read.partial !== undefined ? `STOPPED, ${total} programs judged before` : `${total} programs`;
      console.log(`  ${unit.label.padEnd(8)} ${String(seconds).padStart(4)}s  ${counted} ${JSON.stringify(outcomes)}  ${summary}`);
      if (gridSample) { console.log(`           ${sampleLine(gridSample)}`); sampledUnits.push(unit.label); }
      const log = existsSync(logPath) ? readFileSync(logPath, "utf8") : "";
      attention.push(...unitProblems(unit.label, { run, grid: read, total, log, logPath, timeoutMinutes: options.gridTimeout ?? 120 }));
      if (emptyByDesign({ run, grid: read, total })) console.log(`           no programs: the grid has none for ${unit.codes ? unit.codes.join(",") : "these codes"}${gridShard ? ` in shard ${gridShard.index}/${gridShard.count}` : ""}`);
      if (unit.codes?.length === 1) judgedByCode.set(unit.codes[0], (judgedByCode.get(unit.codes[0]) ?? 0) + total);
      runTotal += total;
      // An executor that cannot start fails every grid the same way: the rest of the run would only repeat it.
      if (total === 0 && /the executor did not start/.test(log)) throw new FocusStop(`${unit.label}: the executor did not start (${log.trim().split("\n").slice(-3).join(" | ").slice(0, 400)})`);
      for (const row of flagged.slice(0, 10)) attention.push(`${unit.label}: ${row.outcome} ${row.id} ${JSON.stringify(row.detail ?? {}).slice(0, 200)}`);
      if (flagged.length > 10) attention.push(`${unit.label}: ... ${flagged.length - 10} more (${join(unit.dir, "programs.jsonl")})`);
      // A full disk shows as `harness` rows; name the cause and stop, for the next unit would fail the same way.
      const filled = diskFilled({ log, flagged });
      if (filled) attention.push(`${unit.label}: the disk filled while the unit ran (ENOSPC in ${logPath} or its rows); its harness rows are that, not the checker. ${noRoom(unitPlaces({ unitDir: unit.dir, scratch: scratchRoot, roleLog: options.roleLog })) ?? "free space is back now"}`);
      for (const result of behaviour.slice(0, 10)) attention.push(`${unit.label}: oracle differs ${result.id} ${result.diffs.map((d) => `${d.kind} ${d.detail}`).join("; ").slice(0, 200)}`);
      // A smoke run: every program whose behaviour moved from the manifest's; a move to a worse outcome needs a look.
      if (options.smoke && existsSync(manifest) && existsSync(join(unit.dir, "programs.jsonl"))) {
        const moved = movedVerdicts(JSON.parse(readFileSync(manifest, "utf8")), readRows(unit.dir));
        // A row on the open-defects ledger is judged there, by id: only a move off it is news.
        moved.worse = offLedger(readOpenDefects(), basename(unit.dir), moved.worse);
        if (moved.worse.length || moved.other.length) console.log(`           moved from the manifest: ${moved.worse.length} worse, ${moved.other.length} better or changed`);
        // A sample holds some of a manifest's programs: they are compared, and the others are not expected.
        if (moved.missing.length && !options.cover) attention.push(`${unit.label}: ${moved.missing.length} programs of the manifest gave no row (${moved.missing.slice(0, 3).join(", ")})`);
        for (const move of moved.worse.slice(0, 10)) attention.push(`${unit.label}: worse ${move.id} ${move.from} -> ${move.to}`);
        if (moved.worse.length > 10) attention.push(`${unit.label}: ... ${moved.worse.length - 10} more worse`);
        for (const move of moved.other.slice(0, 5)) console.log(`             ${move.id} ${move.from} -> ${move.to}`);
      }
      // The unit's perf record, once its fold (if any) has added the role decisions and the fold and prune times.
      let recorded = false;
      const recordPerf = (fold) => {
        if (recorded || !perfOn) return;
        recorded = true;
        const cache = existsSync(join(unit.dir, "cache.json")) ? cacheState(JSON.parse(readFileSync(join(unit.dir, "cache.json"), "utf8"))) : "none";
        const machine = sampler ? machineWindow(sampler.samples(started, ended), stretchOptions) : null;
        const perf = { ...unitPerf({ label: unit.label, dir: unit.dir, startedAt: started, endedAt: ended, programs: total, machine, fold, cache, gridSample, gridShard }), startedAt: started, endedAt: ended };
        if (existsSync(unit.dir)) writeFileSync(join(unit.dir, "perf.json"), `${JSON.stringify(perf, null, 2)}\n`);
        perfUnits.push(perf);
        console.log(`  ${perfLine(perf)}`);
      };
      // The fold and the pruning run in a worker (fold-worker.mjs) while the next grid runs; the run-wide report waits for them all.
      if ((roles && existsSync(join(unit.dir, "programs.jsonl"))) || options.pruneCases) {
        pending.push(new Promise((settle) => {
          const worker = new Worker(join(here, "fold-worker.mjs"), { workerData: { dir: unit.dir, roles, against, pruneCases: Boolean(options.pruneCases) } });
          worker.once("message", (message) => {
            recordPerf({ foldMs: message.foldMs ?? null, pruneMs: message.pruneMs ?? null });
            if (message.summary) console.log(`  ${unit.label}: ${message.summary}`);
            if (message.notInInventory?.length) attention.push(`${unit.label}: role log arms missing from the source inventory: ${message.notInInventory.join(", ")}`);
            if (message.error) attention.push(`${unit.label}: role fold failed: ${message.error}`);
            if (message.folded) folded.push(unit.dir);
          });
          worker.once("error", (error) => { attention.push(`${unit.label}: fold worker failed: ${String(error).slice(0, 200)}`); recordPerf(null); settle(); });
          worker.once("exit", () => { recordPerf(null); settle(); });
        }));
      } else recordPerf(null);
      if (oracle?.inconclusive) attention.push(`${unit.label}: ${oracle.inconclusive} oracle runs inconclusive (${join(unit.dir, "runtime-diff.jsonl")})`);
      if (filled) throw new FocusStop(`${unit.label}: the disk filled, the units after it would fail the same way`);
    };
    try {
    for (const grid of options.grids.split(",")) {
      const [script, ...shape] = GRIDS[grid];
      // A grid with no cell for these codes (a code with no legal context, say) has nothing to run.
      // lg-pairs takes the misuse cells a settled pair is legal in too (legal.mjs pairLegalCells): `--split-codes` must make a unit for
      // their codes (PENDING_ASYNC_FORBIDDEN_SCOPE has no legal cell of its own; its onSettled later-mount pairs were skipped).
      const cells = whole(grid) ? censusCells({ codes }) : script === "legal.mjs" ? (shape.includes("--pairs") ? pairLegalCells({ codes }) : legalCells({ codes, recall: shape.includes("--recall") })) : cellList({ codes, recall: shape.includes("--recall") });
      if (!cells.length) { console.log(`  ${grid.padEnd(8)}    -  no cells for these codes`); continue; }
      const manifest = join(SMOKE_DIR, `${grid}.json`);
      // A covering sample takes a grid's share of it; a share of no program leaves the grid out.
      const chosen = covered?.[grid];
      if (chosen?.ids.length === 0) { console.log(`  ${grid.padEnd(8)}    -  no program of the sample`); continue; }
      // `--split-codes`: one process per code of the grid's cells, in the cells' order.
      const units = options.splitCodes ? [...new Set(cells.map((cell) => cell.code))].map((code) => ({ codes: [code], dir: join(out, grid, code), label: `${grid}/${code}` })) : [{ codes, dir: join(out, grid), label: grid }];
      for (const unit of units) runUnit(grid, script, shape, unit, manifest);
    }
    if (options.splitCodes) for (const [code, judged] of judgedByCode) if (judged === 0) attention.push(`${code}: no grid of the run judged a program of it`);
    if (runTotal === 0) attention.push("the run judged no program");
  } catch (error) {
    if (!(error instanceof FocusStop)) throw error;
    attention.push(`stopped: ${error.message}`);
    process.exitCode = 2;
  }
  await Promise.all(pending);
  if (perfOn) {
    const report = runPerf({ options, out, units: perfUnits, sampler, runStarted, runEnded: Date.now(), stretchOptions });
    sampler?.stop();
    writeFileSync(join(out, "perf.json"), `${JSON.stringify(report, null, 2)}\n`);
    console.log(`\n${runPerfLine(report)} (${join(out, "perf.json")})`);
    for (const stretch of report.machine.idleStretches ?? []) console.log(`  idle ${stretch.seconds}s at +${stretch.atS}s (${Math.round(stretch.idle * 100)}% idle, ${Math.round(stretch.iowait * 100)}% iowait) ${stretch.during}`);
    if (options.perfAgainst) {
      try {
        const result = comparePerf(readRun(options.perfAgainst), report, options.perfThreshold === undefined ? {} : { threshold: Number(options.perfThreshold) });
        console.log("");
        for (const line of compareLines(result)) console.log(line);
        if (!result.comparable) attention.push(`perf: not comparable with ${options.perfAgainst}: ${result.reasons.slice(0, 3).join("; ")}`);
        for (const worse of result.worse) attention.push(`perf: worse than ${options.perfAgainst}: ${worse}`);
      } catch (error) {
        attention.push(`perf: ${error.message}`);
      }
    }
  }
  // Pruned grids left only their empty scratch directories; kept ones (no --prune-cases) still need theirs.
  if (scratchRoot && options.pruneCases) rmSync(scratchRoot, { recursive: true, force: true });
  if (folded.length) {
    // From the grids' roles.json: --prune-cases has deleted their raw logs by now.
    const report = mergeGridReports(folded, { against });
    writeFileSync(join(out, "roles.json"), `${JSON.stringify(report, null, 2)}\n`);
    console.log(`\n${summaryLine(report)} (${join(out, "roles.json")})`);
  }
  if (sampledUnits.length) console.log(`\nsampled (--sample-big ${options.sampleBig}), not a full run: ${sampledUnits.join(", ")}`);
  if (attention.length) {
    console.log(`\nneeds a look (${attention.length}):`);
    for (const line of attention) console.log(`  ${line}`);
    process.exitCode ||= 1;
  } else console.log("\nnothing needs a look");
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) await main();
