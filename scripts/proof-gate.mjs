#!/usr/bin/env bun
// The proof gate (ADR 0933): runs the checker over every fixture project with the proof log on, folds the logs, and holds the
// hits to fixtures/proof-log/gaps.json.
//
// A site of the independent universe (rust/crates/solid-reactive-ir/src/proof_universe.rs) must end in a row, a census obligation,
// a compiler discard or a named silence; a reporting row must end in a finding or a typed drop; a silent row's role must come from
// a tagged, non-default arm of the classifier (scripts/lib/proof-log.mjs describes C1, C2 and C3).
//
//   bun scripts/proof-gate.mjs                     compare against the gap file (exit 1 on a new, stale or invalid entry)
//   bun scripts/proof-gate.mjs --list              print every hit, grouped, with its line and column
//   bun scripts/proof-gate.mjs --report <file>     write every hit as JSON
//   bun scripts/proof-gate.mjs --skeleton <file>   write the hits no entry accepts, `why: "UNREVIEWED"` (never accepted by the gate)
//   --out <dir>   keep the logs there (default: a temporary directory, removed after)
//   --only <text> only the projects whose id contains the text (repeatable)
//
// The gap file only shrinks: a hit it does not list fails, and an entry no hit matches fails. A C2 hit (a reporting decision or
// row with nothing ending it) is a false-clean candidate and is never accepted as a gap; fixtures/proof-log/candidates.json lists
// the ones under review, each naming who fixes it. The gate runs the checker one-shot (SOLID_CHECKER_DAEMON=0): the proof log is
// exact for a fresh build, not for a retained daemon's reused computations.
import { execFile } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, relative, resolve } from "node:path";
import process from "node:process";
import { promisify } from "node:util";

import { fixtureProjects, runtimeArguments } from "./lib/fixture-projects.mjs";
import { gateConcurrency, mapPoolSettled } from "./lib/pool.mjs";
import { CHECKS, gapProblems, hitKey, proofHits, proofStats, readProofLog } from "./lib/proof-log.mjs";

const run = promisify(execFile);
export const ROOT = resolve(import.meta.dirname, "..");
export const GAPS = join(ROOT, "fixtures", "proof-log", "gaps.json");
export const CANDIDATES = join(ROOT, "fixtures", "proof-log", "candidates.json");

function option(argv, name) {
  const at = argv.indexOf(name);
  return at >= 0 ? argv[at + 1] : undefined;
}

function options(argv, name) {
  const values = [];
  for (let at = 0; at < argv.length; at += 1) if (argv[at] === name) values.push(argv[at + 1]);
  return values;
}

/** Prefers a packaged binary, falls back to the debug build cargo leaves behind (as scripts/coverage.mjs does). */
function locate(variable, ...candidates) {
  const override = process.env[variable];
  if (override) return override;
  return candidates.find((candidate) => existsSync(candidate)) ?? candidates[0];
}

/** `line:column` (1-based; the column in bytes) of a byte offset in a file, for a reader. */
function lineColumn(path, offset, cache) {
  if (!cache.has(path)) cache.set(path, existsSync(path) ? readFileSync(path) : Buffer.alloc(0));
  const bytes = cache.get(path);
  let line = 1;
  let start = 0;
  for (let at = 0; at < Math.min(offset, bytes.length); at += 1) {
    if (bytes[at] === 10) {
      line += 1;
      start = at + 1;
    }
  }
  return `${line}:${offset - start + 1}`;
}

const readJson = (path) => (existsSync(path) ? JSON.parse(readFileSync(path, "utf8")) : {});

/** Runs the checker on one project with the proof log into `log`; the hits, stats and checker status of its log. */
async function analyze(project, { checker, typefacts, out }) {
  const log = join(out, `${project.id.replace(/\//g, "__")}.jsonl`);
  rmSync(log, { force: true });
  try {
    await run(checker, ["--format", "json", "--project", project.tsconfig, ...runtimeArguments(project.tsconfig)], {
      cwd: ROOT,
      encoding: "utf8",
      maxBuffer: 256 * 1024 * 1024,
      env: {
        ...process.env,
        SOLID_CHECKER_DAEMON: "0",
        SOLID_TYPEFACTS_BIN: typefacts,
        SOLID_CHECKER_IR_PROOF_LOG: log,
        SOLID_CHECKER_IR_PROOF_LOG_PROGRAM: project.id,
      },
    });
  } catch (error) {
    return { project, failed: String(error.stderr ?? error.message).trim().slice(0, 400) };
  }
  const { lines, malformed, missing } = readProofLog(log);
  if (missing || malformed > 0) return { project, failed: missing ? "the checker wrote no proof log" : `${malformed} malformed proof log line(s)` };
  return { project, hits: proofHits(lines), stats: proofStats(lines) };
}

async function main(argv) {
  const checker = locate("SOLID_CHECKER_BIN", join(ROOT, "bin", "solid-checker-rust"), join(ROOT, "rust", "target", "debug", "solid-checker-rust"));
  const typefacts = locate("SOLID_TYPEFACTS_BIN", join(ROOT, "bin", "solid-typefacts"));
  for (const [name, path] of [["checker", checker], ["type facts producer", typefacts]]) {
    if (!existsSync(path)) {
      console.error(`missing ${name} at ${path} -- run 'make build-rust' first`);
      return 2;
    }
  }
  const only = options(argv, "--only");
  const projects = fixtureProjects(ROOT).filter((project) => only.length === 0 || only.some((text) => project.id.includes(text)));
  if (projects.length === 0) {
    console.error("no fixture projects selected");
    return 2;
  }
  const keep = option(argv, "--out");
  const out = keep ? resolve(keep) : mkdtempSync(join(tmpdir(), "proof-gate-"));
  mkdirSync(out, { recursive: true });
  try {
    const settled = await mapPoolSettled(projects, (project) => analyze(project, { checker, typefacts, out }), { concurrency: gateConcurrency() });
    const failed = [];
    const hitsByKey = new Map();
    const totals = { sites: 0, rows: 0, reportedRows: 0, decisions: 0, silents: 0, verdicts: 0, siteEnds: {}, drops: {}, proofs: {} };
    const cache = new Map();
    for (const [index, result] of settled.entries()) {
      const value = result.error ? { project: projects[index], failed: String(result.error.message ?? result.error) } : result.value;
      if (value.failed) {
        failed.push(`${value.project.id}: ${value.failed}`);
        continue;
      }
      for (const key of ["sites", "rows", "reportedRows", "decisions", "silents", "verdicts"]) totals[key] += value.stats[key];
      for (const group of ["siteEnds", "drops", "proofs"]) {
        for (const [name, count] of Object.entries(value.stats[group])) totals[group][name] = (totals[group][name] ?? 0) + count;
      }
      for (const hit of value.hits) {
        const file = relative(ROOT, hit.file);
        const key = hitKey(value.project.id, file, hit);
        if (!hitsByKey.has(key)) hitsByKey.set(key, { ...hit, key, fixture: value.project.id, relative: file, at: `${file}:${lineColumn(hit.file, hit.s, cache)}` });
      }
    }
    const gaps = readJson(GAPS).gaps ?? {};
    const candidates = readJson(CANDIDATES).candidates ?? {};
    const problems = gapProblems(hitsByKey, gaps, candidates);

    console.log(`proof gate: ${projects.length - failed.length}/${projects.length} projects, ${totals.sites} sites (${Object.entries(totals.siteEnds).map(([end, count]) => `${end} ${count}`).join(", ")}), ${totals.rows} rows (${totals.reportedRows} reported), ${totals.decisions} decisions, ${totals.silents} named silences`);
    if (Object.keys(totals.drops).length > 0) console.log(`drops and demotions: ${Object.entries(totals.drops).sort().map(([pass, count]) => `${pass} ${count}`).join(", ")}`);
    if (argv.includes("--list")) console.log(`named silences: ${Object.entries(totals.proofs).sort().map(([proof, count]) => `${proof} ${count}`).join(", ")}`);
    const byCheck = new Map();
    for (const hit of hitsByKey.values()) {
      const slot = byCheck.get(hit.check) ?? new Map();
      slot.set(hit.kind, (slot.get(hit.kind) ?? 0) + 1);
      byCheck.set(hit.check, slot);
    }
    for (const check of CHECKS) {
      const kinds = byCheck.get(check);
      console.log(`${check}: ${kinds ? [...kinds.values()].reduce((a, b) => a + b, 0) : 0} hit(s)${kinds ? ` (${[...kinds].map(([kind, count]) => `${kind} ${count}`).join(", ")})` : ""}`);
    }
    const byArm = new Map();
    for (const hit of hitsByKey.values()) {
      for (const arm of hit.detail.arms ?? (hit.detail.arm !== undefined ? [hit.detail.arm] : [null])) {
        const name = `${hit.check}/${hit.kind} ${arm ?? "(no arm)"}`;
        byArm.set(name, (byArm.get(name) ?? 0) + 1);
      }
    }
    if (argv.includes("--list")) {
      for (const hit of [...hitsByKey.values()].sort((a, b) => a.check.localeCompare(b.check) || a.kind.localeCompare(b.kind) || a.key.localeCompare(b.key))) {
        console.log(`  ${hit.check}/${hit.kind} ${hit.fixture} ${hit.at} ${hit.table} ${JSON.stringify(hit.detail)}`);
      }
      for (const [name, count] of [...byArm].sort()) console.log(`  by arm: ${name} x${count}`);
    }
    const report = option(argv, "--report");
    if (report) writeFileSync(report, `${JSON.stringify({ version: 1, projects: projects.length, failed, hits: [...hitsByKey.values()] }, null, 2)}\n`);
    const skeleton = option(argv, "--skeleton");
    if (skeleton) {
      const entries = {};
      for (const hit of [...hitsByKey.values()].sort((a, b) => a.key.localeCompare(b.key))) {
        if (!(hit.key in gaps) && hit.check !== "C2") entries[hit.key] = { check: hit.check, why: "UNREVIEWED", at: hit.at, detail: hit.detail };
      }
      writeFileSync(skeleton, `${JSON.stringify({ version: 1, gaps: entries }, null, 2)}\n`);
    }

    for (const line of failed) console.error(`checker failed: ${line}`);
    let failing = failed.length;
    for (const problem of problems) {
      if (problem.kind === "candidate") {
        console.log(`false-clean candidate (under review): ${problem.key} -- ${problem.message}`);
        continue;
      }
      failing += 1;
      const hit = hitsByKey.get(problem.key);
      console.error(`${problem.kind}: ${problem.key}${hit ? ` at ${hit.at}` : ""} -- ${problem.message}`);
    }
    if (failing > 0) {
      console.error(`${failing} problem(s): a new hit is a gap to close or, if it is sound, a stated reason in fixtures/proof-log/gaps.json; a C2 hit is a false-clean candidate for the coordinator`);
      return 1;
    }
    console.log("proof gate: ok");
    return 0;
  } finally {
    if (!keep) rmSync(out, { recursive: true, force: true });
  }
}

if (process.argv[1] && process.argv[1].endsWith("proof-gate.mjs")) process.exit(await main(process.argv.slice(2)));
