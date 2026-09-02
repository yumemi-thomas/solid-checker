#!/usr/bin/env bun
// Relative performance gate: base commit vs head, interleaved on one machine.
//
// The absolute invariants in verify-performance.mjs need shared-runner
// headroom (a 26ms local path measures 67ms there), so they can only catch
// gross regressions. This script closes that gap without an external
// service: it runs the SAME corpus through the base commit's benchmark
// binary and the head's, alternating base/head rounds so scheduler drift
// hits both sides equally, and gates on the ratio of medians.
//
//   bun benchmarks/compare-performance.mjs \
//     --base-bench <path> --base-typefacts <path> \
//     --head-bench <path> --head-typefacts <path> \
//     [--rounds 5]
//
// The regression threshold is a ratio, SOLID_CHECKER_MAX_RELATIVE_REGRESSION
// (default 1.25). Interleaved same-machine medians are stable to a few
// percent: across ten consecutive shared-runner runs in September 2026 the
// ratio stayed within 0.98-1.04 while the absolute numbers on the same runs
// swung 1.6x, so 25% headroom rejects real regressions the absolute ceilings
// cannot see while staying an order of magnitude clear of the noise.

import { appendFileSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";

const repository = resolve(import.meta.dirname, "..");

function argument(name) {
  const index = process.argv.indexOf(name);
  return index === -1 ? undefined : process.argv[index + 1];
}

const sides = {
  base: {
    bench: argument("--base-bench"),
    typefacts: argument("--base-typefacts")
  },
  head: {
    bench: argument("--head-bench"),
    typefacts: argument("--head-typefacts")
  }
};
for (const [side, { bench, typefacts }] of Object.entries(sides)) {
  if (!bench || !typefacts) {
    console.error(`missing --${side}-bench or --${side}-typefacts`);
    process.exit(2);
  }
}
const rounds = Number(argument("--rounds") ?? 5);
const threshold = Number(
  process.env.SOLID_CHECKER_MAX_RELATIVE_REGRESSION ?? 1.25
);

function run(command, arguments_) {
  const result = spawnSync(command, arguments_, {
    cwd: repository,
    encoding: "utf8",
    env: process.env
  });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(
      [result.stderr, result.stdout].filter(Boolean).join("\n").trim() ||
        `${command} exited ${result.status}`
    );
  }
  return result.stdout;
}

const directory = mkdtempSync(join(tmpdir(), "solid-checker-compare-"));
const median = values => {
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.floor(sorted.length / 2)];
};

try {
  const corpus = join(directory, "corpus");
  run(process.execPath, [
    "benchmarks/generate-bench-corpus.mjs",
    "1000",
    corpus
  ]);

  const samples = { base: { incremental: [], firstIr: [] }, head: { incremental: [], firstIr: [] } };
  const measure = side => {
    const report = JSON.parse(
      run(sides[side].bench, [
        "--project",
        join(corpus, "tsconfig.json"),
        "--typefacts",
        sides[side].typefacts,
        "--iterations",
        "10",
        "--warmups",
        "2",
        "--edit",
        join(corpus, "mod0001.tsx"),
        "--edit-mode",
        "same-span-body"
      ])
    );
    samples[side].incremental.push(report.medianNs);
    samples[side].firstIr.push(
      report.firstRustPipelineBreakdown.reactiveIrTotal.medianNs /
        report.sourceCount
    );
  };

  // Alternate sides within each round so a machine slowing down over the run
  // penalizes base and head alike.
  for (let round = 0; round < rounds; round += 1) {
    for (const side of round % 2 === 0 ? ["base", "head"] : ["head", "base"]) {
      measure(side);
    }
  }

  let failed = false;
  const summary = ["| Metric | Base (median) | Head (median) | Ratio | Limit | Verdict |", "| --- | --- | --- | --- | --- | --- |"];
  for (const [metric, unit] of [
    ["incremental", "ns/edit"],
    ["firstIr", "ns/source"]
  ]) {
    const base = median(samples.base[metric]);
    const head = median(samples.head[metric]);
    const ratio = head / Math.max(base, 1);
    const verdict = ratio > threshold ? "REGRESSION" : "ok";
    if (ratio > threshold) failed = true;
    console.log(
      `${metric}: base ${base.toFixed(0)} ${unit}, head ${head.toFixed(0)} ${unit}, ratio ${ratio.toFixed(3)} (limit ${threshold}) ${verdict}`
    );
    // The per-round samples, so a borderline verdict can be read against
    // the spread that produced it rather than the two medians alone.
    console.log(
      `  rounds: base [${samples.base[metric].map(value => value.toFixed(0)).join(", ")}], head [${samples.head[metric].map(value => value.toFixed(0)).join(", ")}]`
    );
    summary.push(`| ${metric} | ${base.toFixed(0)} ${unit} | ${head.toFixed(0)} ${unit} | ${ratio.toFixed(3)} | ${threshold} | ${verdict} |`);
  }
  if (process.env.GITHUB_STEP_SUMMARY) {
    appendFileSync(
      process.env.GITHUB_STEP_SUMMARY,
      `### Base versus head on this runner (${rounds} interleaved rounds)\n\n${summary.join("\n")}\n\n`
    );
  }
  if (failed) {
    console.error(
      `head regresses past ${threshold}x of the base commit on this machine`
    );
    process.exit(1);
  }
} finally {
  rmSync(directory, { recursive: true, force: true });
}
