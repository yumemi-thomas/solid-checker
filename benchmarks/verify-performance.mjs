#!/usr/bin/env bun
// Performance invariants over the deterministic benchmark corpus.
//
//   bun benchmarks/verify-performance.mjs [--wall-time-gate enforce|report]
//
// Structural invariants (export scaling, Type Facts payload, cached reuse) are
// always enforced. The wall-time ceilings (fresh Reactive IR per source, the
// one-file incremental edit) are enforced by default — the right mode for a
// machine you control, `make verify-performance` and `scripts/verify.sh` —
// and only reported with `--wall-time-gate report` (or
// SOLID_CHECKER_WALL_TIME_GATE=report), the mode the Performance workflow uses
// on shared runners, where the same commit measures 1.6x apart between runs
// and the interleaved base-versus-head race is the regression gate. See
// benchmarks/lib/performance-invariants.mjs for the measurements behind that.
//
// Ceiling overrides, for exercising the gate: SOLID_CHECKER_MAX_FIRST_IR_NS_PER_SOURCE,
// SOLID_CHECKER_MAX_INCREMENTAL_NS.

import { appendFileSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";

import {
  DEFAULT_CEILINGS,
  WALL_TIME_GATES,
  assessPerformance,
  renderPerformanceFindings
} from "./lib/performance-invariants.mjs";

const repository = resolve(import.meta.dirname, "..");
const benchmark = join(repository, "rust/target/release/solid-checker-session-bench");
const typefacts = process.env.SOLID_TYPEFACTS_BIN ?? join(repository, "bin/solid-typefacts");

function argument(name) {
  const index = process.argv.indexOf(name);
  return index === -1 ? undefined : process.argv[index + 1];
}
const wallTimeGate = argument("--wall-time-gate") ?? process.env.SOLID_CHECKER_WALL_TIME_GATE ?? "enforce";
if (!WALL_TIME_GATES.includes(wallTimeGate)) {
  console.error(`--wall-time-gate must be one of ${WALL_TIME_GATES.join(", ")}, got ${JSON.stringify(wallTimeGate)}`);
  process.exit(2);
}
const ceilings = {
  ...DEFAULT_CEILINGS,
  ...(process.env.SOLID_CHECKER_MAX_FIRST_IR_NS_PER_SOURCE
    ? { firstIrNsPerSource: Number(process.env.SOLID_CHECKER_MAX_FIRST_IR_NS_PER_SOURCE) }
    : {}),
  ...(process.env.SOLID_CHECKER_MAX_INCREMENTAL_NS
    ? { incrementalNs: Number(process.env.SOLID_CHECKER_MAX_INCREMENTAL_NS) }
    : {})
};

const directory = mkdtempSync(join(tmpdir(), "solid-checker-performance-"));

function run(command, arguments_) {
  const result = spawnSync(command, arguments_, { cwd: repository, encoding: "utf8", env: process.env });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(
      [result.stderr, result.stdout].filter(Boolean).join("\n").trim() || `${command} exited ${result.status}`
    );
  }
  return result.stdout;
}

function measure(files, options = {}) {
  const corpus = join(directory, options.corpus ?? String(files));
  run(process.execPath, ["benchmarks/generate-bench-corpus.mjs", String(files), corpus]);
  return JSON.parse(
    run(benchmark, [
      "--project",
      join(corpus, "tsconfig.json"),
      "--typefacts",
      typefacts,
      "--iterations",
      String(options.iterations ?? 1),
      "--warmups",
      String(options.warmups ?? 0),
      ...(options.edit
        ? ["--edit", join(corpus, options.edit), "--edit-mode", options.editMode ?? "same-span-body"]
        : [])
    ])
  );
}

try {
  const small = measure(500);
  // Three independent cold processes; the gate takes the best, since shared
  // scheduling slows one sample while a real regression slows every one.
  const largeSamples = Array.from({ length: 3 }, (_, index) => measure(1000, { corpus: `large-${index}` }));
  const incremental = measure(1000, { iterations: 10, warmups: 2, edit: "mod0001.tsx" });

  const assessment = assessPerformance({ small, largeSamples, incremental }, { wallTimeGate, ceilings });
  if (process.env.GITHUB_STEP_SUMMARY) {
    appendFileSync(
      process.env.GITHUB_STEP_SUMMARY,
      `### Performance invariants\n\n${renderPerformanceFindings(assessment, { wallTimeGate })}\n`
    );
  }
  for (const finding of assessment.reported) {
    process.stdout.write(`performance ceiling exceeded on this machine (reported, not enforced): ${finding.message}\n`);
  }
  if (!assessment.ok) {
    throw new Error(assessment.violations.map(finding => finding.message).join("; "));
  }
  process.stdout.write(`performance certification passed (wall-time gate: ${wallTimeGate}): ${assessment.summary}\n`);
} finally {
  rmSync(directory, { recursive: true, force: true });
}
