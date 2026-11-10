#!/usr/bin/env bun
// Arm ablation (ADR 0933, the audit's M6): DESIGNED AND WRITTEN, NOT RUN. It is heavy (one checker run per pair below), so it runs only
// when the owner asks for it, under `sc-heavy`, on the release binary.
//
// The question. The classifier's tagged arms are the proofs behind a silent role: C3 (scripts/lib/proof-log.mjs) fails a silent site
// that no tagged, non-default arm answered, but it cannot tell whether an arm that did answer is *right*. An arm that no fixture
// depends on is a claim nothing tests: if its answer were wrong, every gate would stay green (audit S2, row 5). So each arm that
// answers silent somewhere must have a fixture whose findings change when that arm stops answering.
//
// The experiment. For every pair (fixture, arm) where a silent decision of that fixture was answered by the arm, re-run the fixture
// with SOLID_CHECKER_IR_ABLATE_ARM=<arm> (the hook in rust/crates/solid-reactive-ir/src/role_log.rs makes that arm answer `Unknown`,
// an obligation, instead of its role; nothing else changes) and compare the findings with the run without it. An arm is
//
//   witnessed    some fixture's findings change: that fixture fails without the arm, so the arm is tested;
//   unwitnessed  no fixture's findings change: nothing tests it. The arm needs a misuse fixture (phase C) or a premise that says why
//                it cannot be wrong; the report lists it with the fixtures that exercise it.
//
// Cost. One baseline run per fixture and one ablated run per pair: with the 1,092 fixture projects of `make coverage` and the arms
// their silent decisions name, expect thousands of single-project runs (about 0.3 s each on the release binary). `--concurrency`
// bounds them (use 4 with xargs-like pacing, the repository's rule for raw checker loops), `--budget-seconds` stops cleanly and
// `--resume` continues from the report, so the run can be split over several sc-heavy slots.
//
//   bun scripts/proof-ablation.mjs --logs <dir> [--out report.json] [--resume report.json] [--only <text>] [--concurrency 4]
//                                  [--budget-seconds 3000] [--list]
//   --logs <dir>   the directory `bun scripts/proof-gate.mjs --out <dir>` kept: one proof log per project (the pairs come from it)
//   --list         print the pairs and their count, run nothing
//
// Reading the report. `arms[id]` has the fixtures that witness it (`witnesses`) and those that exercise it without changing
// (`inert`); `unwitnessed` lists the arm ids with no witness, `unrun` the pairs a budget stopped before. Neither is a verdict about
// the checker: an unwitnessed arm is a missing test, not a false clean.
import { execFile } from "node:child_process";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import process from "node:process";
import { promisify } from "node:util";

import { fixtureProjects, runtimeArguments } from "./lib/fixture-projects.mjs";
import { mapPoolSettled } from "./lib/pool.mjs";
import { proofLogLines } from "./lib/proof-log.mjs";

const run = promisify(execFile);
const ROOT = resolve(import.meta.dirname, "..");

/**
 * The (fixture, arm) pairs of a proof log's lines: every arm that answered a silent `read`, `write` or `action` decision. Arms the
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
 * Folds the results of ablated runs into the report: `results` are `{ fixture, arm, changed }`. An arm with a changed run is
 * witnessed by those fixtures; an arm whose every run was unchanged is unwitnessed.
 */
export function summarize(results) {
  const arms = {};
  for (const { fixture, arm, changed } of results) {
    arms[arm] ??= { witnesses: [], inert: [] };
    arms[arm][changed ? "witnesses" : "inert"].push(fixture);
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

async function check(project, { checker, typefacts }, ablate) {
  const env = { ...process.env, SOLID_CHECKER_DAEMON: "0", SOLID_TYPEFACTS_BIN: typefacts };
  delete env.SOLID_CHECKER_IR_ABLATE_ARM;
  if (ablate) env.SOLID_CHECKER_IR_ABLATE_ARM = ablate;
  const { stdout } = await run(checker, ["--format", "json", "--project", project.tsconfig, ...runtimeArguments(project.tsconfig)], {
    cwd: ROOT,
    encoding: "utf8",
    maxBuffer: 256 * 1024 * 1024,
    env,
  }).catch((error) => ({ stdout: error.stdout ?? "" }));
  return findingsKey(JSON.parse(stdout || "{}"));
}

async function main(argv) {
  const logs = option(argv, "--logs");
  if (!logs || !existsSync(logs)) {
    console.error("usage: bun scripts/proof-ablation.mjs --logs <dir kept by proof-gate --out> [--out report.json] [--resume report.json] [--list]");
    return 2;
  }
  const only = argv.flatMap((value, at) => (value === "--only" ? [argv[at + 1]] : []));
  const projects = fixtureProjects(ROOT).filter((project) => only.length === 0 || only.some((text) => project.id.includes(text)));
  const pairs = [];
  for (const project of projects) {
    const log = join(logs, `${project.id.replace(/\//g, "__")}.jsonl`);
    if (!existsSync(log)) continue;
    for (const arm of armsOf([...proofLogLines(log)])) pairs.push({ project, arm });
  }
  if (argv.includes("--list")) {
    for (const { project, arm } of pairs) console.log(`${project.id}\t${arm}`);
    console.log(`${pairs.length} pair(s) over ${new Set(pairs.map(({ arm }) => arm)).size} arm(s)`);
    return 0;
  }
  const checker = process.env.SOLID_CHECKER_BIN ?? join(ROOT, "bin", "solid-checker-rust");
  const typefacts = process.env.SOLID_TYPEFACTS_BIN ?? join(ROOT, "bin", "solid-typefacts");
  const out = resolve(option(argv, "--out") ?? "proof-ablation-report.json");
  const resume = option(argv, "--resume");
  const done = new Map();
  for (const result of existsSync(resume ?? "") ? JSON.parse(readFileSync(resume, "utf8")).results : []) done.set(`${result.fixture}\t${result.arm}`, result);
  const deadline = Date.now() + 1000 * Number(option(argv, "--budget-seconds") ?? 3000);
  const baselines = new Map();
  const baseline = async (project) => {
    if (!baselines.has(project.id)) baselines.set(project.id, check(project, { checker, typefacts }, undefined));
    return baselines.get(project.id);
  };
  const todo = pairs.filter(({ project, arm }) => !done.has(`${project.id}\t${arm}`));
  const settled = await mapPoolSettled(
    todo,
    async ({ project, arm }) => {
      if (Date.now() > deadline) return null;
      const [base, ablated] = await Promise.all([baseline(project), check(project, { checker, typefacts }, arm)]);
      return { fixture: project.id, arm, changed: base !== ablated };
    },
    { concurrency: Number(option(argv, "--concurrency") ?? 4) },
  );
  const results = [...done.values(), ...settled.flatMap((result) => (result.value ? [result.value] : []))];
  const unrun = todo.filter((_, at) => !settled[at].value).map(({ project, arm }) => ({ fixture: project.id, arm }));
  writeFileSync(out, `${JSON.stringify({ version: 1, ...summarize(results), unrun, results }, null, 2)}\n`);
  const summary = summarize(results);
  console.log(`proof ablation: ${results.length} pair(s) run, ${unrun.length} left, ${summary.unwitnessed.length} unwitnessed arm(s) of ${Object.keys(summary.arms).length}`);
  for (const arm of summary.unwitnessed) console.log(`  unwitnessed ${arm} (exercised by ${summary.arms[arm].inert.length} fixture(s))`);
  console.log(`report: ${out}`);
  return 0;
}

if (process.argv[1] && process.argv[1].endsWith("proof-ablation.mjs")) process.exit(await main(process.argv.slice(2)));
