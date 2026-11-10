#!/usr/bin/env node
// The uncertifiable rows of the grids' programs, with their typed blockers (ADR 0954): the checker alone, no runtime, on programs
// regenerated from the grids' rows as `make fuzz-focus` generates them. One JSON line per program for `scripts/u-verdicts.mjs`, which
// joins them with the verdicts a run already holds. Nothing here judges: a row's outcome is the grid run's, and this reads none.
//
//   node scripts/fuzz/blocker-scan.mjs --checker-bin <solid-checker-rust> --typefacts <solid-typefacts> --out <fresh rows.jsonl>
//     [--grids mx-recall,lg-recall,...]   every grid by default
//     [--codes A,B] [--contexts a,b]      the cells to scan
//     [--ids <file>]                      only these program ids (a JSON array, `{ "ids": [...] }`, or one id a line)
//     [--sample <fraction> [--sample-seed N]]   a deterministic share of the rows, picked by a hash of the id and the seed
//     [--shard i/N]                       the rows whose id hash falls in shard i of N
//     [--concurrency 12] [--install scripts/fuzz/install] [--seed 1]
//
// The first line is `{ scan: { grids, codes, contexts, ids, sample, sampleSeed, shard, seed, programs } }`, the selection. Each other line:
// `{ id, grid, side, code, context, site: { file, line, region }, u: { total, kinds: { <kind>: [atSite, away] } }, violationsAtSite,
// formulas: [{ rule, kind, formula, fired, missing, line, atSite }] }`; `violationsAtSite` counts the violations standing in the site region. A row is `atSite` when it stands in the program's judged site
// region (matrix.mjs `regionOf`: the site's line and its aux lines) in App.tsx; the judge also covers by a frame Chrome recorded, which
// this run has no record of, so a row at a frame reads `away` here. `kinds` counts every uncertifiable finding by its blocker kind
// (`blockedBy.kind`, else `none`); `formulas` lists the rows whose blocker names a formula or a missing fact, one entry each. A program
// the checker could not answer is `{ id, grid, error }`. The summary on stderr counts programs, rows, errors and the rate.
import { spawn } from "node:child_process";
import { existsSync, readFileSync, realpathSync, rmSync, writeFileSync, appendFileSync } from "node:fs";
import { join, relative, resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { gridRows } from "./cover.mjs";
import { GRIDS } from "./focus-grids.mjs";
import { checkerEnv, materializeForChecker } from "./checker.mjs";
import { generateLegal } from "./legal.mjs";
import { generateShape, regionOf, siteOf } from "./matrix.mjs";
import { fnv } from "./rng.mjs";

const here = dirname(fileURLToPath(import.meta.url));
/** The harness's checker arguments (checker.mjs `CHECKER_ARGS`, `ORDINARY_ARGS`): copied, not imported, so this file never moves the runtime records' key. */
const CHECKER_ARGS = ["--format", "json", "--feedback-facts", "--runtime-target", "browser", "--runtime-build", "development", "--rendering", "csr"];
const ORDINARY_ARGS = ["--format", "json", "--runtime-target", "browser", "--runtime-build", "development", "--rendering", "csr"];

/** The options of an argument list, or throws on one it does not know. */
export function parseOptions(argv) {
  const options = { grids: Object.keys(GRIDS), codes: null, contexts: null, ids: null, sample: null, sampleSeed: "1", shard: null, concurrency: 12, install: join(here, "install"), seed: "1", checker: null, typefacts: null, out: null };
  const names = { "checker-bin": "checker", typefacts: "typefacts", out: "out", grids: "grids", codes: "codes", contexts: "contexts", ids: "ids", sample: "sample", "sample-seed": "sampleSeed", shard: "shard", concurrency: "concurrency", install: "install", seed: "seed" };
  for (let index = 0; index < argv.length; index += 2) {
    const key = names[argv[index].replace(/^--/, "")];
    if (!key || argv[index + 1] === undefined) throw new Error(`unknown or incomplete option ${argv[index]}`);
    options[key] = argv[index + 1];
  }
  for (const name of ["checker", "typefacts", "out"]) if (!options[name]) throw new Error(`--${name === "checker" ? "checker-bin" : name} is required`);
  for (const name of ["grids", "codes", "contexts"]) if (typeof options[name] === "string") options[name] = options[name].split(",").filter(Boolean);
  for (const grid of options.grids) if (!GRIDS[grid]) throw new Error(`unknown grid ${grid} (${Object.keys(GRIDS).join(", ")})`);
  options.concurrency = Number(options.concurrency);
  if (!Number.isInteger(options.concurrency) || options.concurrency < 1) throw new Error("--concurrency takes a positive integer");
  if (options.sample !== null) {
    options.sample = Number(options.sample);
    if (!(options.sample > 0 && options.sample <= 1)) throw new Error("--sample takes a fraction in (0, 1]");
  }
  if (options.shard !== null) {
    const match = /^(\d+)\/(\d+)$/.exec(options.shard);
    if (!match || Number(match[1]) >= Number(match[2])) throw new Error("--shard takes i/N with i < N");
    options.shard = [Number(match[1]), Number(match[2])];
  }
  return options;
}

/** The ids a file names: a JSON array, `{ ids }`, or one id a line. */
export function readIds(path) {
  const text = readFileSync(path, "utf8");
  try {
    const value = JSON.parse(text);
    return new Set(Array.isArray(value) ? value : value.ids);
  } catch {
    return new Set(text.split("\n").map((line) => line.trim()).filter(Boolean));
  }
}

/** Whether `row` is in the selection `options` describe: a pure function of the row's id, so two runs and two machines scan the same programs. */
export function selected(row, options, ids = null) {
  if (options.contexts && !options.contexts.includes(row.context)) return false;
  if (ids && !ids.has(row.id)) return false;
  if (options.sample !== null && fnv(`${options.sampleSeed}:${row.id}`) % 1_000_000 >= options.sample * 1_000_000) return false;
  if (options.shard && fnv(`shard:${row.id}`) % options.shard[1] !== options.shard[0]) return false;
  return true;
}

/** The program of a grid row, as the grid generates it (regen: matrix.mjs `generateShape`, legal.mjs `generateLegal`). */
export function buildRow(row, seed = "1") {
  const { code, context, grid, side, id, ...shape } = row;
  const built = side === "legal" ? generateLegal({ seed }, { code, context }, { index: 0, shape }).built : generateShape({ seed }, { code, context }, shape, 0).built;
  if (built.id !== id) throw new Error(`id mismatch: the row says ${id}, the generator built ${built.id}`);
  return built;
}

/**
 * The line a checker report reduces to: every uncertifiable finding of the program's own files by blocker kind (at the site or away
 * from it), and the rows whose blocker names a formula or a missing fact. `report` is the parsed JSON, `root` the program's real directory.
 */
export function reduceReport(report, root, site) {
  const prefix = `${root}/`;
  const region = new Set(regionOf(site));
  const kinds = {};
  const formulas = [];
  let total = 0;
  let violationsAtSite = 0;
  for (const finding of report.findings ?? []) {
    const path = finding.primaryLocation?.path;
    if (!path || !path.startsWith(prefix)) continue;
    const file = relative(root, path);
    const line = finding.primaryLocation.line;
    const endLine = finding.primaryLocation.endLine ?? line;
    const atSite = file === "App.tsx" && [...region].some((at) => line <= at && at <= endLine);
    if (finding.kind === "violation" && atSite) violationsAtSite += 1;
    if (finding.kind !== "uncertifiable") continue;
    const blocked = finding.blockedBy ?? null;
    const kind = blocked?.kind ?? "none";
    total += 1;
    kinds[kind] ??= [0, 0];
    kinds[kind][atSite ? 0 : 1] += 1;
    if (blocked && (blocked.formula || blocked.missing)) {
      formulas.push({ rule: finding.id, kind, formula: blocked.formula ?? null, fired: [...(blocked.fired ?? [])].sort(), missing: blocked.missing ?? null, line, atSite });
    }
  }
  return { total, violationsAtSite, kinds: Object.fromEntries(Object.entries(kinds).sort(([a], [b]) => (a < b ? -1 : 1))), formulas };
}

function runChecker(dir, { checker, typefacts, ordinary }) {
  return new Promise((done) => {
    const child = spawn(checker, ["--project", join(dir, "tsconfig.json"), ...(ordinary ? ORDINARY_ARGS : CHECKER_ARGS)], {
      cwd: dir, stdio: ["ignore", "pipe", "pipe"], env: { ...checkerEnv({ typefacts }), SOLID_CHECKER_DAEMON: "0" },
    });
    let stdout = "";
    let stderr = "";
    const timer = setTimeout(() => child.kill("SIGKILL"), 180_000);
    child.stdout.on("data", (chunk) => { stdout += chunk; });
    child.stderr.on("data", (chunk) => { stderr += chunk; });
    child.on("error", (error) => { clearTimeout(timer); done({ error: error.message }); });
    child.on("close", (status) => {
      clearTimeout(timer);
      if (status !== 0 && status !== 1) { done({ error: `exit ${status}: ${stderr.slice(-300)}` }); return; }
      try { done({ report: JSON.parse(stdout) }); } catch (error) { done({ error: `malformed output: ${error.message}` }); }
    });
  });
}

async function scan(options) {
  const ids = options.ids ? readIds(options.ids) : null;
  const rows = [];
  for (const grid of options.grids) for (const row of gridRows(grid, options.codes ? { codes: options.codes } : {})) if (selected(row, options, ids)) rows.push(row);
  const out = resolve(options.out);
  if (existsSync(out)) throw new Error(`use a fresh --out path (${out} exists)`);
  const checker = resolve(options.checker);
  const header = { scan: { grids: options.grids, codes: options.codes, contexts: options.contexts, ids: options.ids ? options.ids : null, sample: options.sample, sampleSeed: options.sample === null ? null : options.sampleSeed, shard: options.shard, seed: options.seed, programs: rows.length } };
  writeFileSync(out, `${JSON.stringify(header)}\n`);
  const typefacts = resolve(options.typefacts);
  const stats = { programs: 0, errors: 0, rows: 0, formulaRows: 0 };
  const started = Date.now();
  let next = 0;
  const worker = async () => {
    while (next < rows.length) {
      const row = rows[next++];
      let line;
      try {
        const built = buildRow(row, options.seed);
        const dir = materializeForChecker(built, options.install, built.id);
        try {
          const ordinary = Object.keys(built.packages ?? {}).length > 0;
          const answer = await runChecker(dir, { checker, typefacts, ordinary });
          const site = siteOf(built);
          if (answer.error) line = { id: row.id, grid: row.grid, error: answer.error };
          else {
            const u = reduceReport(answer.report, realpathSync(dir), site);
            stats.rows += u.total;
            stats.formulaRows += u.formulas.length;
            line = { id: row.id, grid: row.grid, side: row.side, code: row.code, context: row.context, site: { file: "App.tsx", line: site.line, region: regionOf(site) }, u: { total: u.total, kinds: u.kinds }, violationsAtSite: u.violationsAtSite, formulas: u.formulas };
          }
        } finally {
          rmSync(dir, { recursive: true, force: true });
        }
      } catch (error) {
        line = { id: row.id, grid: row.grid, error: error instanceof Error ? error.message : String(error) };
      }
      if (line.error) stats.errors += 1;
      stats.programs += 1;
      appendFileSync(out, `${JSON.stringify(line)}\n`);
      if (stats.programs % 2000 === 0) process.stderr.write(`blocker-scan: ${stats.programs}/${rows.length} programs, ${stats.errors} errors, ${((stats.programs * 1000) / (Date.now() - started)).toFixed(0)} programs/s\n`);
    }
  };
  process.stderr.write(`blocker-scan: ${rows.length} programs from ${options.grids.join(",")}${options.codes ? ` (codes ${options.codes.join(",")})` : ""}\n`);
  await Promise.all(Array.from({ length: options.concurrency }, worker));
  const seconds = (Date.now() - started) / 1000;
  process.stderr.write(`blocker-scan: done ${stats.programs} programs in ${seconds.toFixed(0)} s (${(stats.programs / seconds).toFixed(0)}/s): ${stats.rows} uncertifiable rows, ${stats.formulaRows} with a formula or a missing fact, ${stats.errors} errors\n`);
  if (stats.errors) process.exitCode = 1;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) await scan(parseOptions(process.argv.slice(2)));
