#!/usr/bin/env node
// Batch 11, slice 0 (measurement only): who ends a site (scripts/lib/proof-credit.mjs). Reads the proof logs a run kept (`bun scripts/proof-gate.mjs --out <dir>` for the fixtures;
// a corpus or grid run with SOLID_CHECKER_IR_PROOF_LOG=<dir>) and counts, per Chrome runtime code, the sites a claiming rule's violation ends, a declining rule's obligation ends,
// only ANOTHER rule's finding ends (`other`, the any-rule-credit list), and no finding ends. Changes no finding and no gate.
//
//   node scripts/proof-credit.mjs <log dir | log file> [--per-file] [--json]
import { readdirSync, statSync } from "node:fs";
import { join, resolve } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

import { creditSummary, siteCredits } from "./lib/proof-credit.mjs";
import { proofLogLines } from "./lib/proof-log.mjs";

const files = (path) => (statSync(path).isDirectory() ? readdirSync(path).filter((name) => name.endsWith(".jsonl")).sort().map((name) => join(path, name)) : [path]);

/** The credits of every log file under `path`, each file's own (a log names its paths per process), `[{ file, credits }]`. */
export function creditsOf(path) {
  return files(resolve(path)).map((file) => ({ file, credits: siteCredits([...proofLogLines(file)]) }));
}

export function render(summary, title) {
  const lines = [`${title}: ${summary.sites} site(s): claims ${summary.byCredit.claims}, declines ${summary.byCredit.declines}, other ${summary.byCredit.other}, none ${summary.byCredit.none}`];
  for (const [code, entry] of Object.entries(summary.perCode).sort()) lines.push(`  ${code}: ${entry.sites} site(s): claims ${entry.claims}, declines ${entry.declines}, other ${entry.other}, none ${entry.none}`);
  for (const [key, count] of Object.entries(summary.otherBy).sort((a, b) => b[1] - a[1])) lines.push(`  ended only by another rule: ${key}: ${count}`);
  return lines.join("\n");
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const [path, ...flags] = process.argv.slice(2);
  if (!path) {
    process.stderr.write("usage: node scripts/proof-credit.mjs <log dir | log file> [--per-file] [--json]\n");
    process.exit(64);
  }
  const perFile = creditsOf(path);
  const total = creditSummary(perFile.flatMap(({ credits }) => credits));
  if (flags.includes("--json")) {
    process.stdout.write(`${JSON.stringify({ total, ...(flags.includes("--per-file") ? { files: Object.fromEntries(perFile.map(({ file, credits }) => [file, creditSummary(credits)])) } : {}) }, null, 2)}\n`);
  } else {
    if (flags.includes("--per-file")) for (const { file, credits } of perFile) process.stdout.write(`${render(creditSummary(credits), file)}\n`);
    process.stdout.write(`${render(total, "all")}\n`);
  }
}
