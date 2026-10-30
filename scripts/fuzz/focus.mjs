// The fuzzer's short loop (`make fuzz-focus CODES=A,B`): the six grids (misuse and legal; base, sources, combinations) restricted to the
// given codes, on the Rust executor and the Chrome host, each with its oracle sample (one program per stratum in real Chrome through
// `feedback run`). It prints each grid's outcomes and the oracle comparison, lists what needs a look, and exits 1 when anything does:
// a false clean, a wrong rule, a false positive, a legal program the runtime contradicts, a harness failure, or a program whose record
// differs from the oracle's in behaviour. Tens of seconds per code, about half that again when the cache holds the programs. The full
// grids run before a pull request (scripts/fuzz/README.md).
//
//   node scripts/fuzz/focus.mjs --codes A,B --checker-bin <solid-checker-rust> --typefacts <solid-typefacts>
//     [--grids mx-base,mx-src,mx-combo,lg-base,lg-src,lg-combo] [--seed 1] [--out <dir>] [--runtime-diff <per stratum>]
//     [--oracle-axes code,context,outcome] [--smoke] [-- <grid args>]
// `--smoke` (`make fuzz-smoke`): every code, each grid restricted to its smoke manifest (smoke.mjs, scripts/fuzz/smoke/<grid>.json).
// The loop's oracle sample is stratified by code, context and outcome (every outcome of every context in real Chrome); the full run's
// by code, context, source, mount and outcome (runtime-diff.mjs DEFAULT_DIFF_AXES), many more programs. `--oracle-axes` picks.
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { legalCells } from "./legal.mjs";
import { cellList } from "./matrix.mjs";
import { SMOKE_DIR, movedVerdicts, readRows } from "./smoke.mjs";

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
};
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
  const behaviour = (oracle?.differing ?? []).filter((result) => result.diffs.some((d) => d.kind !== "moved"));
  return { outcomes, flagged, oracle: oracle?.summary ?? null, behaviour };
}

function parse(argv) {
  const split = argv.indexOf("--");
  const own = split < 0 ? argv : argv.slice(0, split);
  const options = { grids: Object.keys(GRIDS).join(","), seed: "1", runtimeDiff: "1", oracleAxes: "code,context,outcome", extra: split < 0 ? [] : argv.slice(split + 1) };
  for (let index = 0; index < own.length; index += 2) {
    const key = own[index].replace(/^--/, "").replace(/-([a-z])/g, (_, c) => c.toUpperCase());
    if (key === "smoke") { options.smoke = true; options.codes ??= "all"; index -= 1; continue; }
    if (!["codes", "grids", "seed", "out", "runtimeDiff", "oracleAxes", "checkerBin", "typefacts"].includes(key) || own[index + 1] === undefined) throw new Error(`focus.mjs: unknown or empty option ${own[index]}`);
    options[key] = own[index + 1];
  }
  if (!options.codes) throw new Error("focus.mjs: --codes A,B (the codes the change touches)");
  for (const grid of options.grids.split(",")) if (!GRIDS[grid]) throw new Error(`focus.mjs: no grid ${grid} (${Object.keys(GRIDS).join(", ")})`);
  return options;
}

function main() {
  const options = parse(process.argv.slice(2));
  const out = resolve(options.out ?? join(tmpdir(), "solid-checker-fuzz-focus", `${Date.now()}`));
  mkdirSync(out, { recursive: true });
  const tools = [...(options.checkerBin ? ["--checker-bin", options.checkerBin] : []), ...(options.typefacts ? ["--typefacts", options.typefacts] : [])];
  const oracle = options.runtimeDiff === "0" ? [] : ["--runtime-diff", options.runtimeDiff, "--runtime-diff-axes", options.oracleAxes];
  const host = ["--all-cores", "--fast-host", "chrome", "--chrome-pages", "reuse", "--chrome-clock", "virtual-time", "--executor", "rust", ...oracle];
  const attention = [];
  console.log(`fuzz-${options.smoke ? "smoke" : "focus"}: codes ${options.codes}; output ${out}`);
  // `--codes all`: every code (the full grids); `--runtime-diff 0`: no oracle sample.
  const codes = options.codes === "all" ? undefined : options.codes.split(",");
  for (const grid of options.grids.split(",")) {
    const [script, ...shape] = GRIDS[grid];
    // A grid with no cell for these codes (a code with no legal context, say) has nothing to run.
    const cells = script === "legal.mjs" ? legalCells({ codes }) : cellList({ codes });
    if (!cells.length) { console.log(`  ${grid.padEnd(8)}    -  no cells for these codes`); continue; }
    const started = Date.now();
    const manifest = join(SMOKE_DIR, `${grid}.json`);
    if (options.smoke && !existsSync(manifest)) { attention.push(`${grid}: no smoke manifest ${manifest} (smoke.mjs select)`); continue; }
    const select = [...(codes ? ["--codes", codes.join(",")] : []), ...(options.smoke ? ["--ids", manifest] : [])];
    const run = spawnSync(process.execPath, [join(here, script), "--seed", options.seed, "--grid", ...shape, ...select, "--out", join(out, grid), ...tools, ...host, ...options.extra], { encoding: "utf8", maxBuffer: 1 << 30 });
    const seconds = Math.round((Date.now() - started) / 1000);
    const { outcomes, flagged, oracle, behaviour } = readGrid(join(out, grid));
    const total = Object.values(outcomes).reduce((a, b) => a + b, 0);
    const summary = oracle ? `oracle ${oracle.compared} compared, ${oracle.behaviour} behaviour, ${oracle.attributionOnly} attribution, ${oracle.inconclusive ?? 0} inconclusive` : "no oracle sample";
    console.log(`  ${grid.padEnd(8)} ${String(seconds).padStart(4)}s  ${total} programs ${JSON.stringify(outcomes)}  ${summary}`);
    if (run.status !== 0) attention.push(`${grid}: exited ${run.status} (${join(out, grid)}.log): ${String(run.stderr || run.stdout).trim().split("\n").slice(-3).join(" | ").slice(0, 400)}`);
    for (const row of flagged.slice(0, 10)) attention.push(`${grid}: ${row.outcome} ${row.id} ${JSON.stringify(row.detail ?? {}).slice(0, 200)}`);
    if (flagged.length > 10) attention.push(`${grid}: ... ${flagged.length - 10} more (${join(out, grid, "programs.jsonl")})`);
    for (const result of behaviour.slice(0, 10)) attention.push(`${grid}: oracle differs ${result.id} ${result.diffs.filter((d) => d.kind !== "moved").map((d) => `${d.kind} ${d.detail}`).join("; ").slice(0, 200)}`);
    // A smoke run: every program whose behaviour moved from the manifest's; a move to a worse outcome needs a look.
    if (options.smoke && existsSync(join(out, grid, "programs.jsonl"))) {
      const moved = movedVerdicts(JSON.parse(readFileSync(manifest, "utf8")), readRows(join(out, grid)));
      if (moved.worse.length || moved.other.length) console.log(`           moved from the manifest: ${moved.worse.length} worse, ${moved.other.length} better or changed`);
      if (moved.missing.length) attention.push(`${grid}: ${moved.missing.length} programs of the manifest gave no row (${moved.missing.slice(0, 3).join(", ")})`);
      for (const move of moved.worse.slice(0, 10)) attention.push(`${grid}: worse ${move.id} ${move.from} -> ${move.to}`);
      if (moved.worse.length > 10) attention.push(`${grid}: ... ${moved.worse.length - 10} more worse`);
      for (const move of moved.other.slice(0, 5)) console.log(`             ${move.id} ${move.from} -> ${move.to}`);
    }
    if (oracle?.inconclusive) attention.push(`${grid}: ${oracle.inconclusive} oracle runs inconclusive (${join(out, grid, "runtime-diff.jsonl")})`);
    writeFileSync(join(out, `${grid}.log`), `${run.stdout ?? ""}${run.stderr ?? ""}`);
  }
  if (attention.length) {
    console.log(`\nneeds a look (${attention.length}):`);
    for (const line of attention) console.log(`  ${line}`);
    process.exitCode = 1;
  } else console.log("\nnothing needs a look");
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main();
