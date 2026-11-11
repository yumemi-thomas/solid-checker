#!/usr/bin/env bun
// Arm ablation (ADR 0933, the audit's M6; extended by ADR 0935): it is heavy (one checker run per pair below), so it runs only when the owner asks
// for it, under `sc-heavy`, on a release binary.
//
// The question. The classifier's tagged arms are the proofs behind a silent role: C3 (scripts/lib/proof-log.mjs) fails a silent site
// that no tagged, non-default arm answered, but it cannot tell whether an arm that did answer is *right*. An arm that no program
// depends on is a claim nothing tests: if its answer were wrong, every gate would stay green (audit S2, row 5). So each arm that
// answers silent somewhere must have a program whose findings change when that arm stops answering, and the sites that stand on
// that arm alone must be counted.
//
// The experiment. For every pair (program, arm) where a silent decision of that program was answered by the arm, re-run the program
// with SOLID_CHECKER_IR_ABLATE_ARM=<arm> (the hook in rust/crates/solid-reactive-ir/src/role_log.rs makes that arm answer `Unknown`,
// an obligation, instead of its role; nothing else changes) and compare the findings with the run without it, site by site (a
// site is a finding's span). A site is in one of these transitions:
//
//   cleanToU   no finding at the site, an obligation once the arm is gone: the site stands on this arm alone (its sole proof);
//   cleanToV   no finding at the site, a VIOLATION once the arm is gone: another arm reports where this one is silent, so two arms
//              disagree about the same site. A soundness signal: one of them is wrong, and every one found is listed;
//   vToU       a violation became an obligation (the arm was a reporting one: its proof of the violation);
//   vLost      a violation with nothing at its span afterwards;
//   uToV       an obligation became a violation (another arm reports where this one answered Unknown);
//   uLost      an obligation with nothing at its span afterwards (another arm proved it clean).
//
// An arm is witnessed when some program moves; unwitnessed when none does (a missing test, not a false clean).
//
// Two sources of programs:
//
//   fixtures   the projects `make coverage` runs (the default), their (program, arm) pairs from the proof logs `proof-gate.mjs --out <dir>` kept;
//   a grid     `--grid <dir>`: a focus/matrix run's grid directory with its `cases/` kept (no --prune-cases) and the role log
//              `<dir>.roles.jsonl` (focus.mjs --role-log); the pairs come from the log's silent decisions, the programs from `cases/`.
//
// Cost. One baseline run per program and one ablated run per pair: with the 1,092 fixture projects expect thousands of single-project
// runs (about 0.3 s each on the release binary); a grid, one run per (program, silent arm). `--concurrency` bounds them (use 4 with xargs-like
// pacing, the repository's rule for raw checker loops), `--budget-seconds` stops cleanly and `--resume` continues from the report, so the run
// can be split over several sc-heavy slots.
//
//   bun scripts/proof-ablation.mjs --logs <dir> [--out report.json] [--resume report.json] [--only <text>] [--concurrency 4]
//                                  [--budget-seconds 3000] [--list]
//   bun scripts/proof-ablation.mjs --grid <dir> [--roles <log file|dir>] [--max-per-arm N] [--arms a,b] ...  (the same options)
//   --logs <dir>   the directory `bun scripts/proof-gate.mjs --out <dir>` kept: one proof log per project (the pairs come from it)
//   --grid <dir>   a grid directory with `cases/<id>/` (App.tsx, oracle.json, helpers) and its role log; `SOLID_CHECKER_NATIVE_BIN`
//                  and `SOLID_TYPEFACTS_BIN` (or --checker-bin / --typefacts) name the binaries
//   --max-per-arm  at most N programs per arm (the smallest ids first): a grid has thousands of programs per arm, N bounds the cost
//   --list         print the pairs and their count, run nothing
//
// Reading the report. `arms[id]` counts each transition over its programs and holds examples (`cleanToV` examples in full: every one is a
// disagreement to read); `witnesses` are the programs whose findings moved and `inert` those that did not; `unwitnessed` lists the arm
// ids with no witness, `unrun` the pairs a budget stopped before. None is a verdict about the checker on its own: an unwitnessed arm is a
// missing test, a `cleanToU` count is the size of what stands on the arm, a `cleanToV` is a pair of arms to read.
import { execFile } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import process from "node:process";
import { promisify } from "node:util";

import { AssumedReportError, refuseAssumed } from "./lib/assumed.mjs";
import { fixtureProjects, runtimeArguments } from "./lib/fixture-projects.mjs";
import { mapPoolSettled } from "./lib/pool.mjs";
import { proofLogLines } from "./lib/proof-log.mjs";

const run = promisify(execFile);
const ROOT = resolve(import.meta.dirname, "..");

/**
 * The (program, arm) pairs of a proof log's lines: every arm that answered a silent `read`, `write` or `action` decision. Arms the
 * classifier tags `default` are left out (C3 already fails them), and so are decisions no arm answered. `lines` are the resolved
 * lines of one project's log.
 */
export function armsOf(lines) {
  const arms = new Set();
  for (const line of lines) {
    if (line.k !== "decision" || line.silent !== true) continue;
    if (line.arm === null || line.arm === undefined || line.default === true) continue;
    arms.add(line.arm);
  }
  return [...arms].sort();
}

/** The findings of a checker report as a comparable string: code, kind, blocker and span, in a stable order. */
export function findingsKey(report) {
  return JSON.stringify(
    (report.findings ?? [])
      .map((finding) => [
        finding.id,
        finding.kind,
        finding.analysisContext ?? "",
        finding.primaryLocation?.path ?? "",
        finding.primaryLocation?.startByte ?? 0,
        finding.primaryLocation?.endByte ?? 0,
      ])
      .sort((left, right) => JSON.stringify(left).localeCompare(JSON.stringify(right))),
  );
}

/**
 * A checker report's findings as `{ site, id, kind, blocker }`: `site` is the finding's span (`path:start:end`), the unit a transition is
 * counted in. A finding of the harness's own entry (`main.tsx`) is the entry's business, not a site of the program.
 */
export function findingSites(report) {
  return (report.findings ?? [])
    .filter((finding) => !/\/main\.tsx$/.test(finding.primaryLocation?.path ?? ""))
    .map((finding) => ({
      site: `${finding.primaryLocation?.path ?? ""}:${finding.primaryLocation?.startByte ?? 0}:${finding.primaryLocation?.endByte ?? 0}`,
      id: finding.id,
      kind: finding.kind,
      blocker: String(finding.analysisContext ?? "").split(":")[0],
    }));
}

/** The kind of a site's findings: `V` when any is a violation, `U` when only obligations, `none`. */
const kindOf = (findings) => (findings.some((finding) => finding.kind === "violation") ? "V" : findings.length ? "U" : "none");

/**
 * The transitions of an ablated run against its baseline, site by site (see the header): `{ cleanToU, cleanToV, vToU, vLost, uToV,
 * uLost }`, each a list of `{ site, base, ablated }` with the codes and blockers on each side. Sites whose kind is unchanged are not
 * listed (a different code of the same kind is the same transition: the arm's proof moved, not its verdict).
 */
export function transitions(base, ablated) {
  const bySite = (list) => {
    const sites = new Map();
    for (const finding of list) sites.set(finding.site, [...(sites.get(finding.site) ?? []), finding]);
    return sites;
  };
  const before = bySite(base);
  const after = bySite(ablated);
  const out = { cleanToU: [], cleanToV: [], vToU: [], vLost: [], uToV: [], uLost: [] };
  for (const site of new Set([...before.keys(), ...after.keys()])) {
    const was = kindOf(before.get(site) ?? []);
    const is = kindOf(after.get(site) ?? []);
    if (was === is) continue;
    const entry = {
      site,
      base: (before.get(site) ?? []).map((finding) => `${finding.id}${finding.blocker ? `:${finding.blocker}` : ""}`),
      ablated: (after.get(site) ?? []).map((finding) => `${finding.id}${finding.blocker ? `:${finding.blocker}` : ""}`),
    };
    const name = { "none>U": "cleanToU", "none>V": "cleanToV", "V>U": "vToU", "V>none": "vLost", "U>V": "uToV", "U>none": "uLost" }[`${was}>${is}`];
    out[name].push(entry);
  }
  return out;
}

/** Whether any transition happened. */
export const moved = (result) => Object.values(result).some((list) => list.length > 0);

const KINDS = ["cleanToU", "cleanToV", "vToU", "vLost", "uToV", "uLost"];

/**
 * Folds the results of ablated runs into the report: `results` are `{ fixture, arm, changed, transitions? }`. An arm with a changed
 * run is witnessed by those programs; an arm whose every run was unchanged is unwitnessed. With transitions, each arm also counts
 * them (`counts`, in sites) and keeps examples: the first three of each kind, and every `cleanToV` (two arms disagreeing at a site).
 */
export function summarize(results) {
  const arms = {};
  for (const { fixture, arm, changed, transitions: moves } of results) {
    arms[arm] ??= { witnesses: [], inert: [] };
    arms[arm][changed ? "witnesses" : "inert"].push(fixture);
    if (!moves) continue;
    const entry = (arms[arm].counts ??= Object.fromEntries(KINDS.map((kind) => [kind, 0])));
    const examples = (arms[arm].examples ??= Object.fromEntries(KINDS.map((kind) => [kind, []])));
    for (const kind of KINDS) {
      entry[kind] += moves[kind].length;
      for (const move of moves[kind]) if (kind === "cleanToV" || examples[kind].length < 3) examples[kind].push({ program: fixture, ...move });
    }
  }
  const unwitnessed = Object.keys(arms)
    .filter((arm) => arms[arm].witnesses.length === 0)
    .sort();
  return { arms, unwitnessed };
}

function option(argv, name) {
  const at = argv.indexOf(name);
  return at >= 0 ? argv[at + 1] : undefined;
}

/** One checker run of a fixture project: its findings as sites. */
async function checkProject(project, { checker, typefacts }, ablate) {
  const env = { ...process.env, SOLID_CHECKER_DAEMON: "0", SOLID_TYPEFACTS_BIN: typefacts };
  delete env.SOLID_CHECKER_IR_ABLATE_ARM;
  if (ablate) env.SOLID_CHECKER_IR_ABLATE_ARM = ablate;
  const { stdout } = await run(checker, ["--format", "json", "--project", project.tsconfig, ...runtimeArguments(project.tsconfig)], {
    cwd: ROOT,
    encoding: "utf8",
    maxBuffer: 256 * 1024 * 1024,
    env,
  }).catch((error) => ({ stdout: error.stdout ?? "" }));
  // ADR 0950: a what-if report is a prediction; a transition between two runs of the shipped checker is a measurement. Refused.
  return findingSites(refuseAssumed(JSON.parse(stdout || "{}"), "scripts/proof-ablation.mjs"));
}

/**
 * The (program, arm) pairs of a grid: `{ program, arm }` for every arm that answered a silent decision of the program, from the
 * grid's role log (`roles.mjs` reads the log; the program is the path line's `g` or the checker's work directory).
 */
export async function gridPairs(logPath, { arms, maxPerArm } = {}) {
  const { roleLogLines, programOf } = await import("./fuzz/roles.mjs");
  const perProgram = new Map();
  for (const line of roleLogLines(logPath)) {
    if (!["read", "write", "action"].includes(line.k) || line.silent !== true || !line.arm || line.default === true) continue;
    if (arms && !arms.includes(line.arm)) continue;
    const { program } = programOf(line.f, line.g);
    if (!program) continue;
    if (!perProgram.has(program)) perProgram.set(program, new Set());
    perProgram.get(program).add(line.arm);
  }
  const pairs = [];
  const taken = new Map();
  for (const program of [...perProgram.keys()].sort()) {
    for (const arm of [...perProgram.get(program)].sort()) {
      if (maxPerArm !== undefined && (taken.get(arm) ?? 0) >= maxPerArm) continue;
      taken.set(arm, (taken.get(arm) ?? 0) + 1);
      pairs.push({ program, arm });
    }
  }
  return pairs;
}

/** The whole run, exported for its test; the file runs it only as the entry (below). Rejects with an `AssumedReportError` when a checker prints a what-if report (ADR 0950). */
export async function main(argv) {
  const gridDir = option(argv, "--grid");
  const logs = option(argv, "--logs");
  if (!gridDir && (!logs || !existsSync(logs))) {
    console.error("usage: bun scripts/proof-ablation.mjs (--logs <dir kept by proof-gate --out> | --grid <focus grid dir>) [--out report.json] [--resume report.json] [--list]");
    return 2;
  }
  const only = argv.flatMap((value, at) => (value === "--only" ? [argv[at + 1]] : []));
  const armFilter = option(argv, "--arms")?.split(",");
  const maxPerArm = option(argv, "--max-per-arm") === undefined ? undefined : Number(option(argv, "--max-per-arm"));
  const pairs = [];
  let materialize;
  const checker = resolve(option(argv, "--checker-bin") ?? process.env.SOLID_CHECKER_BIN ?? process.env.SOLID_CHECKER_NATIVE_BIN ?? join(ROOT, "bin", "solid-checker-rust"));
  const typefacts = resolve(option(argv, "--typefacts") ?? process.env.SOLID_TYPEFACTS_BIN ?? join(ROOT, "bin", "solid-typefacts"));
  if (gridDir) {
    const dir = resolve(gridDir);
    const roles = option(argv, "--roles") ?? (existsSync(`${dir}.roles.jsonl`) ? `${dir}.roles.jsonl` : `${dir}.roles`);
    if (!existsSync(roles)) {
      console.error(`proof ablation: no role log for ${dir} (looked for ${roles}); run the grid with focus.mjs --role-log`);
      return 2;
    }
    const cases = join(dir, "cases");
    if (!existsSync(cases)) {
      console.error(`proof ablation: ${cases} is missing: run the grid without --prune-cases`);
      return 2;
    }
    const { materializeFromDirectory, runChecker } = await import("./fuzz/checker.mjs");
    const install = resolve(option(argv, "--install") ?? join(ROOT, "scripts", "fuzz", "install"));
    for (const pair of await gridPairs(roles, { arms: armFilter, maxPerArm })) {
      if (only.length === 0 || only.some((text) => pair.program.includes(text))) {
        if (existsSync(join(cases, pair.program))) pairs.push({ ...pair, project: { id: pair.program, caseDir: join(cases, pair.program) } });
      }
    }
    materialize = async (project, ablate, slot) => {
      const work = materializeFromDirectory(project.caseDir, install, `ablate-${slot}-${project.id}`);
      try {
        const env = { ...process.env };
        delete env.SOLID_CHECKER_IR_ABLATE_ARM;
        if (ablate) env.SOLID_CHECKER_IR_ABLATE_ARM = ablate;
        const result = await runChecker(work, { bin: checker, typefacts, env });
        // The same sites as a fixture's: a line span stands for the byte span (the unit is only compared with its own kind).
        return (result.findings ?? []).map((finding) => ({ site: `${finding.file}:${finding.line}:${finding.column}:${finding.endLine}`, id: finding.id, kind: finding.kind, blocker: finding.blocker ?? "" }));
      } finally {
        rmSync(work, { recursive: true, force: true });
      }
    };
  } else {
    const projects = fixtureProjects(ROOT).filter((project) => only.length === 0 || only.some((text) => project.id.includes(text)));
    for (const project of projects) {
      const log = join(logs, `${project.id.replace(/\//g, "__")}.jsonl`);
      if (!existsSync(log)) continue;
      for (const arm of armsOf([...proofLogLines(log)])) if (!armFilter || armFilter.includes(arm)) pairs.push({ program: project.id, arm, project });
    }
    materialize = (project, ablate) => checkProject(project, { checker, typefacts }, ablate);
  }
  if (argv.includes("--list")) {
    for (const { program, arm } of pairs) console.log(`${program}\t${arm}`);
    console.log(`${pairs.length} pair(s) over ${new Set(pairs.map(({ arm }) => arm)).size} arm(s)`);
    return 0;
  }
  const out = resolve(option(argv, "--out") ?? "proof-ablation-report.json");
  const resume = option(argv, "--resume");
  const done = new Map();
  for (const result of existsSync(resume ?? "") ? JSON.parse(readFileSync(resume, "utf8")).results : []) done.set(`${result.fixture}\t${result.arm}`, result);
  const deadline = Date.now() + 1000 * Number(option(argv, "--budget-seconds") ?? 3000);
  const baselines = new Map();
  const baseline = async (project, slot) => {
    if (!baselines.has(project.id)) baselines.set(project.id, materialize(project, undefined, slot));
    return baselines.get(project.id);
  };
  let slot = 0;
  const todo = pairs.filter(({ program, arm }) => !done.has(`${program}\t${arm}`));
  const settled = await mapPoolSettled(
    todo,
    async ({ program, arm, project }) => {
      if (Date.now() > deadline) return null;
      slot += 1;
      const [base, ablated] = await Promise.all([baseline(project, slot), materialize(project, arm, slot)]);
      const moves = transitions(base, ablated);
      return { fixture: program, arm, changed: moved(moves), transitions: moves };
    },
    { concurrency: Number(option(argv, "--concurrency") ?? 4) },
  );
  // ADR 0950: `mapPoolSettled` keeps a pair's error unread, and an unread one is counted as an "unrun" pair: a refused (what-if) report would
  // end as "N left". It is thrown here instead, and no report is written.
  const refused = settled.find((result) => result.error instanceof AssumedReportError);
  if (refused) throw refused.error;
  const results = [...done.values(), ...settled.flatMap((result) => (result.value ? [result.value] : []))];
  const unrun = todo.filter((_, at) => !settled[at].value).map(({ program, arm }) => ({ fixture: program, arm }));
  const summary = summarize(results);
  mkdirSync(resolve(out, ".."), { recursive: true });
  writeFileSync(out, `${JSON.stringify({ version: 2, ...summary, unrun, results }, null, 2)}\n`);
  console.log(`proof ablation: ${results.length} pair(s) run, ${unrun.length} left, ${summary.unwitnessed.length} unwitnessed arm(s) of ${Object.keys(summary.arms).length}`);
  for (const arm of summary.unwitnessed) console.log(`  unwitnessed ${arm} (exercised by ${summary.arms[arm].inert.length} program(s))`);
  for (const [arm, entry] of Object.entries(summary.arms).sort()) {
    if (!entry.counts) continue;
    const counts = KINDS.filter((kind) => entry.counts[kind]).map((kind) => `${kind} ${entry.counts[kind]}`).join(", ");
    if (counts) console.log(`  ${arm}: ${counts} (${entry.witnesses.length} of ${entry.witnesses.length + entry.inert.length} program(s) move)`);
  }
  const disagreements = Object.entries(summary.arms).flatMap(([arm, entry]) => (entry.examples?.cleanToV ?? []).map((example) => ({ arm, ...example })));
  console.log(`clean to violation (two arms disagree): ${disagreements.length}`);
  for (const example of disagreements) console.log(`  ${example.arm} at ${example.program} ${example.site}: baseline ${example.base.join(",") || "clean"} -> ${example.ablated.join(",")}`);
  console.log(`report: ${out}`);
  return 0;
}

if (process.argv[1] && process.argv[1].endsWith("proof-ablation.mjs")) process.exit(await main(process.argv.slice(2)));
