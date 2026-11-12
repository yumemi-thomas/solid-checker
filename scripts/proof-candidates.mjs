#!/usr/bin/env node
// Batch 11, slice 0 (measurement only): how the candidate sites of the rules that have no site universe end (scripts/lib/candidates.mjs). Reads the proof logs of a run made with
// SOLID_CHECKER_IR_PROOF_LOG=<dir> and SOLID_CHECKER_IR_PROOF_CANDIDATES=1 (one log file per project) and counts, per rule and per project, the candidates and how each ends today:
// a finding of the rule, another rule's finding only, a named silence, the compiler's discard, or nothing (`unaccounted`: what a closed accounting would have to end, an upper bound of the
// uncertifiable rows it would add). Changes no finding and no gate.
//
//   node scripts/proof-candidates.mjs <log dir | log file> [--per-project] [--json]
//   node scripts/proof-candidates.mjs <log dir | log file> --list     every candidate with its would-be ending, one JSON line each (file, program, rule, table, span, facts, end, the rules that ended it)
import { readdirSync, statSync } from "node:fs";
import { join, resolve } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

import { candidatesOf, foldLogFile, mergeTally, newTally } from "./lib/candidates.mjs";
import { proofLogLines } from "./lib/proof-log.mjs";

const files = (path) => (statSync(path).isDirectory() ? readdirSync(path).filter((name) => name.endsWith(".jsonl")).sort().map((name) => join(path, name)) : [path]);

/**
 * The per-project tallies of every log under `path` and their total: `{ projects: { program: { "<rule> <table>": tally } }, total: { "<rule> <table>": tally }, coverage, logs }`.
 * `logs` counts the log files and those holding at least one `candidate` line of the dry run: a run whose analyses stopped early (unresolved imports) has few, and its tallies are
 * a fraction of the truth.
 */
export function foldDirectory(path) {
  const projects = {};
  const total = {};
  const coverage = { findings: {}, uncovered: {}, packageAtEntry: {} };
  const logs = { files: 0, withCandidateLines: 0 };
  for (const file of files(resolve(path))) {
    const { program, tallies, orphans, candidateLines } = foldLogFile(file);
    logs.files += 1;
    if (candidateLines > 0) logs.withCandidateLines += 1;
    const name = program ?? file;
    projects[name] = tallies;
    for (const [key, tally] of Object.entries(tallies)) mergeTally((total[key] ??= newTally()), tally);
    for (const part of ["findings", "uncovered", "packageAtEntry"]) for (const [rule, count] of Object.entries(orphans[part])) coverage[part][rule] = (coverage[part][rule] ?? 0) + count;
  }
  return { projects, total, coverage, logs };
}

const ends = (tally) => `finding ${tally.ends["finding-violation"] + tally.ends["finding-obligation"]} (violation ${tally.ends["finding-violation"]}, obligation ${tally.ends["finding-obligation"]}), other ${tally.ends.other}, silent ${tally.ends.silent}, discarded ${tally.ends.discarded}, unaccounted ${tally.ends.unaccounted}`;

/** The text of a set of tallies: a line per `<rule> <table>`, then the split by each fact and the rules that end sites only as another rule. */
export function render(tallies, title, coverage = null) {
  const lines = [title];
  if (coverage) {
    lines.push(`  the rules' own findings at no candidate (of all their findings at the project's own code): ${Object.keys(coverage.findings).sort().map((rule) => `${rule} ${coverage.uncovered[rule] ?? 0} of ${coverage.findings[rule]}`).join(", ") || "none"}`);
    lines.push(`  findings about a dependency package's own behaviour, reported where project code enters it (no candidate: the package's sites are outside the list): ${Object.entries(coverage.packageAtEntry ?? {}).sort().map(([rule, count]) => `${rule} ${count}`).join(", ") || "none"}`);
  }
  for (const [key, tally] of Object.entries(tallies).sort()) {
    lines.push(`  ${key}: ${tally.candidates} candidate(s): ${ends(tally)}${tally.inTestFiles.candidates ? ` [in test files: ${tally.inTestFiles.candidates}, unaccounted ${tally.inTestFiles.ends.unaccounted}]` : ""}`);
    for (const [fact, entry] of Object.entries(tally.byFact).sort()) lines.push(`      ${fact}: ${entry.candidates}: ${ends(entry)}`);
    for (const [rule, count] of Object.entries(tally.otherBy).sort((a, b) => b[1] - a[1])) lines.push(`      ended only by ${rule}: ${count}`);
  }
  return lines.join("\n");
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const [path, ...flags] = process.argv.slice(2);
  if (!path) {
    process.stderr.write("usage: node scripts/proof-candidates.mjs <log dir | log file> [--per-project] [--json] [--list]\n");
    process.exit(64);
  }
  if (flags.includes("--list")) {
    for (const file of files(resolve(path))) for (const candidate of candidatesOf(proofLogLines(file))) process.stdout.write(`${JSON.stringify(candidate)}\n`);
    process.exit(0);
  }
  const { projects, total, coverage, logs } = foldDirectory(path);
  if (flags.includes("--json")) {
    process.stdout.write(`${JSON.stringify({ total, coverage, logs, ...(flags.includes("--per-project") ? { projects } : {}) }, null, 2)}\n`);
  } else {
    if (flags.includes("--per-project")) for (const [program, tallies] of Object.entries(projects)) process.stdout.write(`${render(tallies, program)}\n`);
    process.stdout.write(`${render(total, `all (${logs.files} log(s), ${logs.withCandidateLines} with a candidate line)`, coverage)}\n`);
  }
}
