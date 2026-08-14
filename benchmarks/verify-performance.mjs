#!/usr/bin/env node

import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";

const repository = resolve(import.meta.dirname, "..");
const benchmark = join(
  repository,
  "rust/target/release/solid-checker-session-bench"
);
const typefacts =
  process.env.SOLID_TYPEFACTS_BIN ?? join(repository, "bin/solid-typefacts");
const directory = mkdtempSync(join(tmpdir(), "solid-checker-performance-"));

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

function measure(files, options = {}) {
  const corpus = join(directory, options.corpus ?? String(files));
  run(process.execPath, [
    "benchmarks/generate-bench-corpus.mjs",
    String(files),
    corpus
  ]);
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
        ? [
            "--edit",
            join(corpus, options.edit),
            "--edit-mode",
            options.editMode ?? "same-span-body"
          ]
        : [])
    ])
  );
}

try {
  const small = measure(500);
  const largeSamples = Array.from({ length: 3 }, (_, index) =>
    measure(1000, { corpus: `large-${index}` })
  );
  const large = largeSamples[0];
  const incremental = measure(1000, {
    iterations: 10,
    warmups: 2,
    edit: "mod0001.tsx"
  });
  const exportTime = report =>
    report.firstRustPipelineBreakdown.reactiveIr.interproceduralExportSummaries
      .medianNs;
  const scaling = exportTime(large) / Math.max(exportTime(small), 1);
  if (scaling > 2.8) {
    throw new Error(
      `Package contract export aggregation scales ${scaling.toFixed(2)}x when the corpus doubles; expected at most 2.8x`
    );
  }

  const responseBytes =
    small.firstAnalysisBreakdown.responseBytes.medianNs / small.sourceCount;
  if (responseBytes > 1000) {
    throw new Error(
      `first Type Facts response uses ${responseBytes.toFixed(0)} bytes/source; expected at most 1000`
    );
  }

  const firstIrSamples = largeSamples.map(
    report =>
      report.firstRustPipelineBreakdown.reactiveIrTotal.medianNs /
      report.sourceCount
  );
  const firstIrPerSource = Math.min(...firstIrSamples);
  // GitHub's shared ubuntu-24.04 runners have measured this cold path at
  // 158us/source while the same revision is about 55us/source locally. Keep
  // enough runner headroom to reject a real regression. Use the best of three
  // independent cold processes so shared-runner scheduling cannot make an
  // otherwise healthy build fail; a sustained regression still exceeds the
  // ceiling in every sample. compare-performance.mjs races the merge base on
  // the same runner for the relative PR comparison.
  const maximumFirstIrPerSource = Number(
    process.env.SOLID_CHECKER_MAX_FIRST_IR_NS_PER_SOURCE ?? 175_000
  );
  if (firstIrPerSource > maximumFirstIrPerSource) {
    throw new Error(
      `best first Reactive IR analysis uses ${firstIrPerSource.toFixed(0)} ns/source; expected at most ${maximumFirstIrPerSource}; samples: ${firstIrSamples.map(sample => sample.toFixed(0)).join(", ")}`
    );
  }

  const cachedIr = small.rustPipelineBreakdown.reactiveIrTotal.medianNs;
  if (cachedIr > 50_000) {
    throw new Error(
      `cached Reactive IR analysis takes ${cachedIr} ns; expected shared-result reuse within 50000 ns`
    );
  }

  // Shared ubuntu-24.04 runners have measured this retained edit path at
  // 67ms while A/B runs of the same revisions are about 26ms locally. Keep
  // this absolute invariant as a gross-regression guard with enough runner
  // headroom; compare-performance.mjs supplies the relative comparison for
  // smaller changes by racing the merge base on the same runner.
  const maximumIncremental = Number(
    process.env.SOLID_CHECKER_MAX_INCREMENTAL_NS ?? 100_000_000
  );
  if (incremental.medianNs > maximumIncremental) {
    throw new Error(
      `one-file incremental analysis takes ${incremental.medianNs} ns; expected at most ${maximumIncremental} ns`
    );
  }

  process.stdout.write(
    `performance certification passed: export scaling ${scaling.toFixed(2)}x, Type Facts ${responseBytes.toFixed(0)} bytes/source, best first IR ${firstIrPerSource.toFixed(0)} ns/source (${firstIrSamples.map(sample => sample.toFixed(0)).join(", ")}), cached IR ${cachedIr} ns, incremental ${incremental.medianNs} ns\n`
  );
} finally {
  rmSync(directory, { recursive: true, force: true });
}
