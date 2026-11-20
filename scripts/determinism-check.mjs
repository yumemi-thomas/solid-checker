#!/usr/bin/env node
// Verdict determinism (ADR 1128): one project through the checker under every worker count and claim order, and repeated; one report each,
// byte for byte the same. A verdict that follows the scheduler (which analysis worker a start landed on, what that worker ran before) differs
// in one run in some number; `SOLID_CHECKER_IR_CLAIM_ORDER` makes the same breach a certain one, with no scheduler.
//
//   node scripts/determinism-check.mjs --project <tsconfig.json> [--checker <bin>] [--typefacts <bin>] [--workers 1,default]
//     [--orders forward,reverse,rotate:7,shuffle:1] [--repeats 2] [--args "<more checker arguments>"] [--cwd <dir>] [--timeout 900] [--out <report.json>]
//   node scripts/determinism-check.mjs --corpus [--manifest scripts/corpus/rc21-manifest.json] [--apps-root <dir>] [--apps a,b] [same options]
//
// A run is one `(workers, order)` and `--repeats` of it (default 2: threads claim as the scheduler lets them, so the repeats of `default`
// workers are the check without an order). The first run is the reference; any run whose output differs fails, with the findings that moved.
// `--corpus` checks every project of a corpus manifest (the manifest's own runtime configuration, each project from its app's root) and prints
// one line per project. A run that exits 1 (the checker's exit for a report with violations) is a run; any other non-zero exit fails it. Exit
// 0 when every run answers alike, 1 when one differs or fails, 2 on a bad call.
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

import { DEFAULT_APPS_ROOT } from "./lib/corpus-manifest.mjs";

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "..");

/** The key of a finding in a comparison: its whole content (a row's message names its accessor, so a rename is a move). */
const rowKey = (finding) => JSON.stringify(finding);

/**
 * What differs between two checker outputs (JSON texts): `{ same, first, other, onlyFirst, onlyOther }`, the counts of findings and the findings
 * only one of them holds (a finding moved is one only in each). Output that is not a report compares as text.
 */
export function compareReports(first, other) {
  if (first === other) return { same: true, first: null, other: null, onlyFirst: [], onlyOther: [] };
  let a;
  let b;
  try {
    a = JSON.parse(first).findings;
    b = JSON.parse(other).findings;
  } catch {
    return { same: false, first: null, other: null, onlyFirst: [], onlyOther: [] };
  }
  const keysA = new Set(a.map(rowKey));
  const keysB = new Set(b.map(rowKey));
  return {
    same: false,
    first: a.length,
    other: b.length,
    onlyFirst: a.filter((finding) => !keysB.has(rowKey(finding))),
    onlyOther: b.filter((finding) => !keysA.has(rowKey(finding))),
  };
}

/** `forward`, `reverse`, `rotate:<n>`, `shuffle:<seed>` (the checker's own spellings); anything else is a mistake in the call. */
export function validOrder(order) {
  return order === "forward" || order === "reverse" || /^rotate:\d+$/.test(order) || /^shuffle:\d+$/.test(order);
}

/** The runs of a call: each `(workers, order)` `repeats` times, the first being the reference. */
export function planRuns(workers, orders, repeats) {
  const runs = [];
  for (const worker of workers) for (const order of orders) for (let repeat = 1; repeat <= repeats; repeat += 1) runs.push({ workers: worker, order, repeat });
  return runs;
}

function describe(finding) {
  const at = finding.primaryLocation ? `${finding.primaryLocation.path}:${finding.primaryLocation.startByte}` : "?";
  return `${finding.id ?? finding.rule} ${finding.kind} ${at} ${String(finding.message ?? "").slice(0, 100)}`;
}

function parse(argv) {
  const options = {
    project: null, corpus: false, manifest: join(repo, "scripts/corpus/rc21-manifest.json"), "apps-root": DEFAULT_APPS_ROOT, apps: null,
    checker: join(repo, "rust/target/release/solid-checker-rust"), typefacts: join(repo, "bin/solid-typefacts"),
    workers: "1,default", orders: "forward,reverse,rotate:7,shuffle:1", repeats: "2", args: "", cwd: process.cwd(), timeout: "900", out: null,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const key = argv[index].replace(/^--/, "");
    if (key === "corpus") { options.corpus = true; continue; }
    if (!(key in options) || argv[index + 1] === undefined) throw new Error(`unknown or incomplete option ${argv[index]}`);
    options[key] = argv[(index += 1)];
  }
  if (!options.project && !options.corpus) throw new Error("--project <tsconfig.json> or --corpus is required");
  const workers = options.workers.split(",").filter(Boolean);
  for (const worker of workers) if (worker !== "default" && !/^[1-9]\d*$/.test(worker)) throw new Error(`--workers takes counts or default, not ${worker}`);
  const orders = options.orders.split(",").filter(Boolean);
  for (const order of orders) if (!validOrder(order)) throw new Error(`--orders takes forward, reverse, rotate:<n> or shuffle:<seed>, not ${order}`);
  const repeats = Number(options.repeats);
  if (!Number.isInteger(repeats) || repeats < 1) throw new Error("--repeats takes a count");
  return { ...options, workers, orders, repeats, timeout: Number(options.timeout) * 1000 };
}

/**
 * The runs of one project. `{ failed, results }`; each run is logged through `log` (`quiet` keeps only the lines of a run that failed or
 * differs).
 */
export function checkProject(options, project, cwd, args, spawn, log, quiet = false) {
  const results = [];
  let reference = null;
  let failed = false;
  for (const run of planRuns(options.workers, options.orders, options.repeats)) {
    const env = { ...process.env, SOLID_CHECKER_DAEMON: "0", SOLID_TYPEFACTS_BIN: options.typefacts, SOLID_CHECKER_IR_CLAIM_ORDER: run.order };
    if (run.workers === "default") delete env.SOLID_CHECKER_IR_WORKERS; else env.SOLID_CHECKER_IR_WORKERS = run.workers;
    const started = Date.now();
    const child = spawn(options.checker, ["--format", "json", "--project", project, ...args], {
      env, cwd, encoding: "utf8", maxBuffer: 1 << 30, timeout: options.timeout,
    });
    const seconds = Math.round((Date.now() - started) / 100) / 10;
    const label = `workers ${run.workers}, claims ${run.order}, run ${run.repeat}`;
    if ((child.status !== 0 && child.status !== 1) || child.error) {
      failed = true;
      log(`FAIL ${label}: ${child.error ? child.error.message : `exit ${child.status}`} ${String(child.stderr ?? "").trim().split("\n").slice(-2).join(" | ")}`);
      results.push({ ...run, status: "failed" });
      continue;
    }
    const hash = createHash("sha256").update(child.stdout).digest("hex").slice(0, 10);
    if (reference === null) {
      reference = { text: child.stdout, label, hash };
      if (!quiet) log(`ref  ${label}: ${hash} (${seconds}s)`);
      results.push({ ...run, status: "reference", hash, seconds });
      continue;
    }
    const diff = compareReports(reference.text, child.stdout);
    if (diff.same) {
      if (!quiet) log(`same ${label}: ${hash} (${seconds}s)`);
      results.push({ ...run, status: "same", hash, seconds });
      continue;
    }
    failed = true;
    log(`DIFF ${label}: ${hash} against ${reference.hash} (${reference.label}); ${diff.first ?? "?"} findings against ${diff.other ?? "?"}`);
    for (const finding of diff.onlyFirst.slice(0, 5)) log(`       only in the reference: ${describe(finding)}`);
    for (const finding of diff.onlyOther.slice(0, 5)) log(`       only in this run:      ${describe(finding)}`);
    results.push({ ...run, status: "differs", hash, seconds, onlyReference: diff.onlyFirst.length, onlyRun: diff.onlyOther.length });
  }
  return { failed, results, reference: reference?.hash ?? null };
}

export function main(argv = process.argv.slice(2), spawn = spawnSync, log = console.log) {
  let options;
  try {
    options = parse(argv);
  } catch (error) {
    console.error(`determinism-check: ${error.message}`);
    return 2;
  }
  for (const path of [options.checker, options.typefacts, ...(options.corpus ? [options.manifest] : [options.project])]) {
    if (!existsSync(path)) { console.error(`determinism-check: ${path} does not exist`); return 2; }
  }
  const extra = options.args.split(/\s+/).filter(Boolean);
  const report = { checker: options.checker, workers: options.workers, orders: options.orders, repeats: options.repeats, projects: [] };
  let failed = false;
  if (!options.corpus) {
    const outcome = checkProject(options, options.project, options.cwd, extra, spawn, log);
    failed = outcome.failed;
    report.projects.push({ project: options.project, ...outcome });
    log(failed ? "determinism-check: FAILED" : `determinism-check: ${outcome.results.length} runs answer alike`);
  } else {
    const manifest = JSON.parse(readFileSync(options.manifest, "utf8"));
    const selected = options.apps ? new Set(options.apps.split(",")) : null;
    const configuration = manifest.configuration;
    const corpusArgs = ["--runtime-target", configuration.runtimeTarget, "--runtime-build", configuration.runtimeBuild, "--rendering", configuration.rendering];
    let total = 0;
    for (const row of manifest.projects.filter((entry) => !selected || selected.has(entry.app))) {
      const cwd = join(options["apps-root"], row.root);
      const project = join(cwd, row.project);
      if (!existsSync(project)) { console.error(`determinism-check: missing project input ${project}`); return 2; }
      const outcome = checkProject(options, project, cwd, [...corpusArgs, ...extra], spawn, log, true);
      total += outcome.results.length;
      failed ||= outcome.failed;
      const seconds = outcome.results.reduce((sum, run) => sum + (run.seconds ?? 0), 0).toFixed(0);
      log(`${outcome.failed ? "FAIL" : "same"} ${row.app}/${row.project}: ${outcome.results.length} runs, ${outcome.reference ?? "-"} (${seconds}s)`);
      report.projects.push({ app: row.app, project: row.project, ...outcome });
    }
    log(failed ? "determinism-check: FAILED" : `determinism-check: ${report.projects.length} projects, ${total} runs answer alike`);
  }
  if (options.out) writeFileSync(options.out, JSON.stringify(report, null, 1) + "\n");
  return failed ? 1 : 0;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) process.exitCode = main();
