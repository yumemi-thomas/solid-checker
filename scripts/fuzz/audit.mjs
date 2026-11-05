#!/usr/bin/env node
// The full fuzz run, split across machines (.github/workflows/fuzz-audit.yml; scripts/fuzz/README.md, "The audit on CI"). The same
// programs and the same oracle sample as one `focus.mjs --codes all` run of the eight grids, in three phases:
//
//   node scripts/fuzz/audit.mjs shard  --shard i/N --out <dir> --checker-bin <bin> --typefacts <bin> [--recall lean|full] [--grid-timeout <min>] [-- <grid args>]
//       focus.mjs on every code and grid restricted to shard i of N (shard.mjs), without the runtime differential: each grid writes
//       the programs the differential would take from its rows (`oracle-candidates.jsonl`, matrix.mjs `writeOracleCandidates`).
//       The recall grids are lean (matrix.mjs LEAN_DEMOTED) unless `--recall full` (the weekly full audit). `pick` writes the run's
//       mode into each manifest, `oracle` runs the manifest's mode, and `report` records it in audit.json (`recall`).
//   node scripts/fuzz/audit.mjs pick   --shards <dir> --out <dir>
//       one `--ids` manifest per grid (`<grid>.json`): the oracle sample of the whole grid, from every shard's candidates, and the
//       oracle jobs (`jobs.json`, one per grid and part: the workflow's matrix).
//   node scripts/fuzz/audit.mjs oracle --grid <grid> --manifest <file> --out <dir> --checker-bin <bin> --typefacts <bin> [--part i/N] [-- <grid args>]
//       the grid on the manifest's programs with the differential on every one of them (Chrome against the fast path). `--part i/N`:
//       part i of the manifest's `parts` (`oracleParts`: the samples are split over the run's runners by their sizes, every N-th id from i).
//       `report` reads a grid's parts from `<oracle>/<grid>/part-<i>/` (a manifest without `parts`: `<oracle>/<grid>/`).
//   node scripts/fuzz/audit.mjs report --shards <dir> --oracle <dir> --out <dir> [--against <rows dir>]
//       the run's verdict: every shard present and whole, what needs a look (focus.mjs PROBLEMS, oracle behaviour differences, an
//       inconclusive or missing oracle program, an outcome that differs between a shard and its oracle grid), and the programs whose
//       outcome moved from `--against` (an earlier report's `rows/`; its recall grids only against an earlier audit of the same mode). Writes audit.json, audit.md and rows/<grid>.jsonl.gz; exit 1
//       when something needs a look.
//
// The oracle sample of a whole grid takes, per stratum, the rows with a fast record in the order of `fnv(id)` (runtime-diff.mjs
// `sampleStrata`); a shard's candidates are the same over its own rows, so the N smallest of all shards' candidates of a stratum are the
// whole grid's N (`pickOracle`). The Chrome work then runs once, on the programs a whole run compares, not once per shard.
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { gunzipSync, gzipSync } from "node:zlib";
import { GRIDS, hostArgs, parse as parseFocus } from "./focus.mjs";
import { rowProblem } from "./codes.mjs";
import { gridStanding, heldLine, readOpenDefects } from "./open-defects.mjs";
import { GENERATOR_VERSION } from "./generate.mjs";
import { BEHAVIOUR_KINDS, parsePerStratum } from "./runtime-diff.mjs";
import { NODE } from "../lib/node-runtime.mjs";

const here = dirname(fileURLToPath(import.meta.url));
/** Every grid, in focus.mjs's order, the pair grids too: a pair axis no run read cost ADR 0585 about three months (ADR 0615). */
export const AUDIT_GRIDS = Object.keys(GRIDS);
/** The oracle sample a focus run takes (focus.mjs defaults): per stratum, and the stratum's axes. */
const FOCUS_DEFAULTS = parseFocus(["--codes", "all"]);
export const ORACLE_PER_STRATUM = FOCUS_DEFAULTS.runtimeDiff;
export const ORACLE_AXES = FOCUS_DEFAULTS.oracleAxes;
const SEED = FOCUS_DEFAULTS.seed;

const readJsonl = (path) => (existsSync(path) ? readFileSync(path, "utf8").split("\n").filter(Boolean).map((line) => JSON.parse(line)) : []);
const readJson = (path) => (existsSync(path) ? JSON.parse(readFileSync(path, "utf8")) : null);
/** A grid directory's report (matrix.json or legal.json). */
const gridReport = (dir) => readJson(join(dir, "matrix.json")) ?? readJson(join(dir, "legal.json"));
const subdirs = (dir) => (existsSync(dir) ? readdirSync(dir, { withFileTypes: true }).filter((entry) => entry.isDirectory()).map((entry) => entry.name).sort() : []);

/**
 * The oracle jobs one run may take, the account's concurrent runners (20, the most run 41775917761 ran at once), and the part size a
 * grid is not split below (ADR 0620). GitHub starts a matrix's jobs in no fixed order (that run's shard 0 started 29th), so a job past
 * the runners waits whatever its size: the split keeps every part a job of its own.
 */
export const ORACLE_RUNNERS = 20;
export const ORACLE_PART_FLOOR = 200;
/**
 * Each grid's oracle parts from its sample's size (`{ grid: programs }`, in AUDIT_GRIDS order): one each, then one more part at a time
 * to the grid whose part is the largest (the first such grid on a tie) while the run has runners left and that part is over the floor.
 * A program costs about the same in every grid (1.0 to 2.3 s of a job's differential, the same code 1.6x apart between runners), so
 * the largest part is the run's tail: mx-census's 2,294 programs in one job took 90 minutes, every other job under 24.
 */
export function oracleParts(sizes) {
  const grids = Object.keys(sizes);
  const parts = Object.fromEntries(grids.map((grid) => [grid, 1]));
  const part = (grid) => Math.ceil(sizes[grid] / parts[grid]);
  for (let jobs = grids.length; grids.length && jobs < ORACLE_RUNNERS; jobs += 1) {
    const largest = grids.reduce((a, b) => (part(b) > part(a) ? b : a));
    if (part(largest) <= ORACLE_PART_FLOOR) break;
    parts[largest] += 1;
  }
  return parts;
}
/** Part `part` of `parts` of a manifest's ids: every `parts`-th from `part`, so each part takes the whole rank order's spread. */
export const partIds = (ids, part, parts) => ids.filter((_, index) => index % parts === part);
/** The oracle jobs of a run, the workflow's matrix, from pick's manifests (`{ grid, parts }`): one per grid and part. */
export const oracleJobs = (manifests) => ({ include: manifests.flatMap(({ grid, parts }) => Array.from({ length: parts }, (_, part) => ({ grid, part, parts }))) });

/** `i/N` as `{ part, parts }`, `0/1` when absent. Throws on anything else. */
export function parsePart(value) {
  if (value === undefined) return { part: 0, parts: 1 };
  const match = /^(\d+)\/(\d+)$/.exec(String(value));
  const [part, parts] = match ? [Number(match[1]), Number(match[2])] : [NaN, NaN];
  if (!(parts >= 1 && part < parts)) throw new Error(`audit.mjs oracle: --part is i/N with 0 <= i < N, not ${value}`);
  return { part, parts };
}

/** The grids whose programs a recall mode changes. */
const RECALL_GRIDS = AUDIT_GRIDS.filter((grid) => grid.endsWith("-recall"));

/**
 * The recall mode of a run's shards, from the recall grids' reports (`recallLean`; a report without it is from before the lean default,
 * so full): `lean`, `full`, or `mixed` when the shards or the two recall grids disagree.
 */
export function recallModeOf(shards) {
  const modes = new Set();
  for (const shard of shards) for (const grid of RECALL_GRIDS) {
    const report = gridReport(join(shard.dir, grid));
    if (report) modes.add(report.recallLean === true ? "lean" : "full");
  }
  return modes.size > 1 ? "mixed" : [...modes][0] ?? "full";
}

/**
 * The whole grid's oracle sample from the shards' candidate lists: per stratum the `perStratum` candidates of smallest `(rank, id)`.
 * @param {Array<Array<{ id: string, stratum: string, rank: number }>>} lists one list per shard
 * @returns {string[]} the picked ids, in the order the differential takes them
 */
export function pickOracle(lists, perStratum) {
  const byStratum = new Map();
  for (const candidate of lists.flat()) {
    if (!byStratum.has(candidate.stratum)) byStratum.set(candidate.stratum, []);
    byStratum.get(candidate.stratum).push(candidate);
  }
  const picked = [];
  for (const candidates of byStratum.values()) picked.push(...candidates.sort(byRank).slice(0, perStratum));
  return picked.sort(byRank).map((candidate) => candidate.id);
}
const byRank = (a, b) => a.rank - b.rank || (a.id < b.id ? -1 : a.id > b.id ? 1 : 0);

/** The shards of a run directory (one directory per shard, each with a directory per grid), checked whole: `{ count, shards, attention }`. */
export function readShards(dir) {
  const attention = [];
  const shards = subdirs(dir).map((name) => ({ name, dir: join(dir, name) }));
  let count = null;
  const seen = new Map();
  for (const shard of shards) {
    for (const grid of AUDIT_GRIDS) {
      const report = gridReport(join(shard.dir, grid));
      if (!report) { attention.push(`${shard.name}: no report for ${grid}`); continue; }
      const part = report.gridShard;
      if (!part) { attention.push(`${shard.name}/${grid}: not a shard`); continue; }
      count ??= part.count;
      if (part.count !== count) attention.push(`${shard.name}/${grid}: shard of ${part.count}, the run's are of ${count}`);
      const key = `${grid}#${part.index}`;
      if (seen.has(key)) attention.push(`${shard.name}/${grid}: shard ${part.index} also in ${seen.get(key)}`);
      seen.set(key, shard.name);
    }
  }
  if (count !== null) for (const grid of AUDIT_GRIDS) for (let index = 0; index < count; index += 1) if (!seen.has(`${grid}#${index}`)) attention.push(`${grid}: shard ${index}/${count} missing`);
  if (!shards.length) attention.push(`no shard in ${dir}`);
  return { count, shards, attention };
}

function pick(options) {
  const { shards, attention } = readShards(resolve(options.shards));
  if (attention.length) throw new Error(`audit pick: the shards are not a whole run:\n  ${attention.join("\n  ")}`);
  const recall = recallModeOf(shards);
  if (recall === "mixed") throw new Error("audit pick: the shards are of both lean and full recall grids (--recall)");
  const out = resolve(options.out);
  mkdirSync(out, { recursive: true });
  const perStratum = parsePerStratum(ORACLE_PER_STRATUM);
  const picked = AUDIT_GRIDS.map((grid) => {
    const lists = shards.map((shard) => readJsonl(join(shard.dir, grid, "oracle-candidates.jsonl")));
    return { grid, lists, ids: pickOracle(lists, perStratum) };
  });
  const split = oracleParts(Object.fromEntries(picked.map(({ grid, ids }) => [grid, ids.length])));
  const manifests = [];
  for (const { grid, lists, ids } of picked) {
    const parts = split[grid];
    manifests.push({ grid, parts });
    writeFileSync(join(out, `${grid}.json`), `${JSON.stringify({ grid, generator: GENERATOR_VERSION, seed: Number(SEED), recall, perStratum: ORACLE_PER_STRATUM, axes: ORACLE_AXES, parts, ids }, null, 2)}\n`);
    console.log(`${grid}: ${ids.length} oracle programs in ${parts} part(s) from ${lists.reduce((sum, list) => sum + list.length, 0)} candidates of ${shards.length} shards`);
  }
  // The oracle matrix (the workflow reads it into its `oracle` job): beside the manifests, never one of them.
  writeFileSync(join(out, "jobs.json"), `${JSON.stringify(oracleJobs(manifests))}\n`);
}

function shard(options, extra) {
  const tools = ["--checker-bin", options.checkerBin, "--typefacts", options.typefacts];
  const args = [join(here, "focus.mjs"), "--codes", "all", "--grids", AUDIT_GRIDS.join(","), "--shard", options.shard, "--runtime-diff", "0", "--prune-cases",
    "--out", resolve(options.out), ...tools, ...(options.recall === "full" ? ["--recall-full"] : []), ...(options.gridTimeout ? ["--grid-timeout", options.gridTimeout] : []),
    "--", "--oracle-candidates", ORACLE_PER_STRATUM, "--runtime-diff-axes", ORACLE_AXES, ...extra];
  return spawnSync(NODE, args, { stdio: "inherit" }).status ?? 1;
}

function oracle(options, extra) {
  const manifest = readJson(resolve(options.manifest));
  if (!manifest) throw new Error(`audit oracle: no manifest ${options.manifest}`);
  if (manifest.grid !== options.grid) throw new Error(`audit oracle: ${options.manifest} is ${manifest.grid}'s, not ${options.grid}'s`);
  const { part, parts } = parsePart(options.part);
  if (parts !== (manifest.parts ?? 1)) throw new Error(`audit oracle: ${options.manifest} is split in ${manifest.parts ?? 1} parts, not ${parts}`);
  const out = resolve(options.out);
  const ids = partIds(manifest.ids, part, parts);
  const label = parts > 1 ? `${options.grid} part ${part}/${parts}` : options.grid;
  if (!ids.length) { mkdirSync(out, { recursive: true }); console.log(`${label}: no oracle program`); return 0; }
  // A part's own manifest, outside `out` (the artifact holds the results, the run's manifests are their own artifact).
  const partManifest = join(mkdtempSync(join(tmpdir(), "fuzz-audit-part-")), `${options.grid}.json`);
  writeFileSync(partManifest, `${JSON.stringify({ ...manifest, part, ids })}\n`);
  const [script, ...shape] = GRIDS[options.grid];
  const heap = options.grid.endsWith("-recall") ? ["--max-old-space-size=12000"] : [];
  // The manifest's ids name the programs of the shards' mode: a recall grid regenerates them in it (a manifest from before `recall` is full).
  const recall = options.grid.endsWith("-recall") && manifest.recall !== "lean" ? ["--recall-full"] : [];
  const args = [...heap, join(here, script), "--seed", SEED, "--grid", ...shape, ...recall, "--ids", partManifest, "--runtime-diff", "all", "--runtime-diff-axes", manifest.axes,
    "--out", out, "--checker-bin", options.checkerBin, "--typefacts", options.typefacts, ...hostArgs(), ...extra];
  console.log(`${label}: the runtime differential on ${ids.length} of the grid's ${manifest.ids.length} oracle programs`);
  return spawnSync(NODE, args, { stdio: "inherit" }).status ?? 1;
}

/** Summary fields a grid's oracle parts add up (runtime-diff.mjs `summarize`, and `inconclusive`). */
const SUMMED = ["compared", "same", "differ", "behaviour", "attributionOnly", "inconclusive"];

/**
 * A grid's oracle results, its parts put together: the summary added up (per kind too), the differing programs and the rows of every part,
 * and what is absent (`<grid>: no oracle result ...`). A part with no program writes nothing and is not absent.
 */
export function readOracle(oracleDir, grid, manifest) {
  const parts = manifest?.parts;
  const dirs = parts ? Array.from({ length: parts }, (_, part) => join(oracleDir, grid, `part-${part}`)) : [join(oracleDir, grid)];
  const absent = [];
  const found = [];
  dirs.forEach((dir, part) => {
    const expected = manifest ? (parts ? partIds(manifest.ids, part, parts).length : manifest.ids.length) : 0;
    const diff = readJson(join(dir, "runtime-diff.json"));
    if (diff) found.push({ dir, diff });
    else if (expected) absent.push(parts > 1 ? `${grid}: no oracle result for part ${part}/${parts} (${expected} programs)` : `${grid}: no oracle result for ${expected} programs`);
  });
  if (!found.length) return { diff: null, oracleRows: [], programRows: [], absent };
  const summary = { ...found[0].diff.summary, byKind: {} };
  for (const key of SUMMED) if (found.some(({ diff }) => diff.summary?.[key] !== undefined)) summary[key] = found.reduce((sum, { diff }) => sum + (diff.summary?.[key] ?? 0), 0);
  for (const { diff } of found) for (const [kind, n] of Object.entries(diff.summary?.byKind ?? {})) summary.byKind[kind] = (summary.byKind[kind] ?? 0) + n;
  return {
    diff: { summary, differing: found.flatMap(({ diff }) => diff.differing ?? []) },
    oracleRows: found.flatMap(({ dir }) => readJsonl(join(dir, "runtime-diff.jsonl"))),
    programRows: found.flatMap(({ dir }) => readJsonl(join(dir, "programs.jsonl"))),
    absent,
  };
}

/**
 * The run's verdict from its shard and oracle directories, and the moves from an earlier report's rows.
 * @returns {{ attention: string[], grids: object, moves: object, rows: Map<string, Map<string, string>> }}
 */
export function judge({ shards: shardsDir, oracle: oracleDir, against }) {
  const { shards, count, attention } = readShards(shardsDir);
  const recall = recallModeOf(shards);
  if (recall === "mixed") attention.push("the shards' recall grids are of both lean and full modes");
  const grids = {};
  const rows = new Map();
  for (const grid of AUDIT_GRIDS) {
    const held = [];
    const knownOpen = readOpenDefects();
    const outcomes = new Map();
    const tally = {};
    let planned = null;
    for (const shard of shards) {
      const dir = join(shard.dir, grid);
      const report = gridReport(dir);
      if (!report?.gridShard) continue;
      planned ??= report.gridShard.of;
      if (report.gridShard.of !== planned) attention.push(`${shard.name}/${grid}: a grid of ${report.gridShard.of} programs, the other shards' of ${planned}`);
      const shardRows = readJsonl(join(dir, "programs.jsonl"));
      if (shardRows.length !== report.gridShard.kept) attention.push(`${shard.name}/${grid}: ${shardRows.length} rows for ${report.gridShard.kept} programs`);
      for (const row of shardRows) {
        if (outcomes.has(row.id)) attention.push(`${grid}: ${row.id} in two shards`);
        outcomes.set(row.id, row.outcome);
        tally[row.outcome] = (tally[row.outcome] ?? 0) + 1;
        const problem = rowProblem(row);
        const standing = gridStanding(knownOpen, row, problem);
        if (standing?.kind === "held") held.push(standing.entry);
        else if (standing?.kind === "fixed") attention.push(`${grid}: open-defect-fixed ${row.id}: move ${standing.entry.item} from open-defects.json to rediscovery.json`);
        else if (problem) attention.push(`${grid}: ${problem} ${row.id} ${JSON.stringify(row.detail ?? {}).slice(0, 200)}`);
      }
    }
    if (held.length) console.log(heldLine(grid, held));
    if (planned !== null && outcomes.size !== planned) attention.push(`${grid}: ${outcomes.size} programs judged of ${planned}`);
    if (planned !== null && planned > 0 && outcomes.size === 0) attention.push(`${grid}: no program judged`);
    // The oracle grid: its sample compared in Chrome, every program of the manifest, each with the outcome its shard gave it.
    const manifest = readJson(join(oracleDir, "manifests", `${grid}.json`));
    const { diff, oracleRows, programRows, absent } = readOracle(oracleDir, grid, manifest);
    let oracle = null;
    if (!manifest) attention.push(`${grid}: no oracle manifest`);
    else attention.push(...absent);
    // The parts that wrote a result are judged even when another is absent: a difference in one is not hidden by a lost runner.
    if (manifest && diff) {
      const compared = new Set(oracleRows.map((result) => result.id));
      const missing = manifest.ids.filter((id) => !compared.has(id));
      if (missing.length) attention.push(`${grid}: ${missing.length} oracle programs not compared (${missing.slice(0, 3).join(", ")})`);
      for (const result of oracleRows) if (result.inconclusive) attention.push(`${grid}: oracle inconclusive ${result.id}: ${String(result.inconclusive).slice(0, 160)}`);
      for (const result of diff.differing ?? []) if (result.diffs.some((d) => BEHAVIOUR_KINDS.includes(d.kind))) attention.push(`${grid}: oracle differs ${result.id} ${result.diffs.map((d) => `${d.kind} ${d.detail}`).join("; ").slice(0, 200)}`);
      for (const row of programRows) {
        if (outcomes.has(row.id) && outcomes.get(row.id) !== row.outcome) attention.push(`${grid}: ${row.id} is ${outcomes.get(row.id)} in its shard, ${row.outcome} in the oracle grid`);
      }
      oracle = { ...diff.summary, programs: manifest.ids.length };
    }
    grids[grid] = { programs: outcomes.size, planned, outcomes: tally, oracle };
    rows.set(grid, outcomes);
  }
  const moves = { against: against ?? null, moved: 0, kinds: {}, examples: [] };
  if (against) {
    // Like with like: the recall grids hold other programs lean than full (matrix.mjs LEAN_DEMOTED), so a lean audit is compared with an
    // earlier lean one, a full one with a full one; the six base grids hold the same programs in both. An earlier audit with no
    // audit.json beside its rows, or with no `recall`, is from before the lean default: full.
    const earlier = readJson(join(dirname(against), "audit.json"))?.recall ?? "full";
    for (const [grid, outcomes] of rows) {
      if (RECALL_GRIDS.includes(grid) && earlier !== recall) { moves.kinds[`${grid}: not compared (the earlier audit was ${earlier}, this one is ${recall})`] = 1; continue; }
      const path = join(against, `${grid}.jsonl.gz`);
      if (!existsSync(path)) { moves.kinds[`${grid}: no earlier rows`] = 1; continue; }
      const before = new Map(gunzipSync(readFileSync(path)).toString("utf8").split("\n").filter(Boolean).map((line) => { const row = JSON.parse(line); return [row.id, row.outcome]; }));
      for (const [id, outcome] of outcomes) {
        const was = before.get(id) ?? "new";
        if (was === outcome) continue;
        moves.moved += 1;
        const kind = `${grid}: ${was} -> ${outcome}`;
        moves.kinds[kind] = (moves.kinds[kind] ?? 0) + 1;
        if (moves.examples.length < 30) moves.examples.push(`${grid} ${id}: ${was} -> ${outcome}`);
      }
      for (const id of before.keys()) if (!outcomes.has(id)) { moves.moved += 1; moves.kinds[`${grid}: gone`] = (moves.kinds[`${grid}: gone`] ?? 0) + 1; }
    }
  }
  return { count, recall, attention, grids, moves, rows };
}

/** The report as Markdown (the workflow's step summary). */
export function markdown({ count, recall, attention, grids, moves }) {
  const lines = [`# Fuzz audit`, "", `${count ?? "?"} shards; ${recall ?? "full"} recall grids; ${Object.values(grids).reduce((sum, grid) => sum + grid.programs, 0)} programs judged.`, ""];
  lines.push("| grid | programs | outcomes | oracle compared | behaviour | inconclusive |", "|---|---|---|---|---|---|");
  for (const [grid, g] of Object.entries(grids)) lines.push(`| ${grid} | ${g.programs} | ${Object.entries(g.outcomes).map(([k, v]) => `${k} ${v}`).join(", ")} | ${g.oracle?.compared ?? "-"} | ${g.oracle?.behaviour ?? "-"} | ${g.oracle?.inconclusive ?? "-"} |`);
  lines.push("", attention.length ? `## Needs a look (${attention.length})` : "Nothing needs a look.");
  for (const item of attention.slice(0, 100)) lines.push(`- ${item}`);
  if (attention.length > 100) lines.push(`- ... ${attention.length - 100} more (audit.json)`);
  if (moves.against) {
    lines.push("", `## Moved from the earlier run: ${moves.moved}`);
    for (const [kind, n] of Object.entries(moves.kinds).sort((a, b) => b[1] - a[1]).slice(0, 40)) lines.push(`- ${kind}: ${n}`);
    for (const example of moves.examples) lines.push(`  - ${example}`);
  }
  return `${lines.join("\n")}\n`;
}

function report(options) {
  const out = resolve(options.out);
  mkdirSync(join(out, "rows"), { recursive: true });
  const result = judge({ shards: resolve(options.shards), oracle: resolve(options.oracle), against: options.against ? resolve(options.against) : undefined });
  for (const [grid, outcomes] of result.rows) {
    const text = [...outcomes].sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0)).map(([id, outcome]) => JSON.stringify({ id, outcome })).join("\n");
    writeFileSync(join(out, "rows", `${grid}.jsonl.gz`), gzipSync(`${text}\n`));
  }
  const { rows, ...summary } = result;
  writeFileSync(join(out, "audit.json"), `${JSON.stringify(summary, null, 2)}\n`);
  const md = markdown(result);
  writeFileSync(join(out, "audit.md"), md);
  console.log(md);
  return result.attention.length ? 1 : 0;
}

/** `audit.mjs <phase> --key value ... [-- <grid args>]`. */
export function parseArgs(argv) {
  const [phase, ...rest] = argv;
  const split = rest.indexOf("--");
  const own = split < 0 ? rest : rest.slice(0, split);
  const extra = split < 0 ? [] : rest.slice(split + 1);
  const options = {};
  for (let index = 0; index < own.length; index += 2) {
    const key = own[index].replace(/^--/, "").replace(/-([a-z])/g, (_, c) => c.toUpperCase());
    if (own[index + 1] === undefined) throw new Error(`audit.mjs: ${own[index]} needs a value`);
    options[key] = own[index + 1];
  }
  const need = { shard: ["shard", "out", "checkerBin", "typefacts"], pick: ["shards", "out"], oracle: ["grid", "manifest", "out", "checkerBin", "typefacts"], report: ["shards", "oracle", "out"] }[phase];
  if (!need) throw new Error("audit.mjs shard|pick|oracle|report (see the file comment)");
  for (const key of need) if (!options[key]) throw new Error(`audit.mjs ${phase}: --${key.replace(/[A-Z]/g, (c) => `-${c.toLowerCase()}`)} is required`);
  if (options.recall !== undefined && !["lean", "full"].includes(options.recall)) throw new Error(`audit.mjs: --recall is lean or full, not ${options.recall}`);
  if (phase === "oracle" && !GRIDS[options.grid]) throw new Error(`audit.mjs oracle: no grid ${options.grid}`);
  if (phase === "oracle") parsePart(options.part);
  return { phase, options, extra };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const { phase, options, extra } = parseArgs(process.argv.slice(2));
  if (phase === "pick") pick(options);
  else process.exitCode = { shard, oracle, report: (o) => report(o) }[phase](options, extra);
}
