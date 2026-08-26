#!/usr/bin/env bun

import { resolve } from "node:path";
import { spawnSync } from "node:child_process";

const repository = resolve(import.meta.dirname, "..");
const benchmark = resolve(
  process.env.SOLID_CHECKER_SESSION_BENCH ??
    `${repository}/rust/target/release/solid-checker-session-bench`,
);
const typefacts = resolve(
  process.env.SOLID_TYPEFACTS_BIN ?? `${repository}/bin/solid-typefacts`,
);
const corpus = resolve(
  process.env.SOLID_CHECKER_CODSPEED_CORPUS ??
    "/tmp/solid-checker-codspeed-corpus",
);
const scenario = process.argv[2];

const common = [
  "--project",
  `${corpus}/tsconfig.json`,
  "--typefacts",
  typefacts,
];
// CodSpeed measures the wall time of this whole process, so each scenario has
// to run its own phase long enough to dominate process start-up and the one
// fresh analysis every session pays before anything can be reused.
const scenarios = {
  fresh: ["--iterations", "1", "--warmups", "0"],
  // A cache hit costs microseconds, so 4,000 of them were about two percent of
  // the process and a reuse regression stayed inside the fresh analysis' noise.
  cached: ["--iterations", "100000", "--warmups", "3"],
  incremental: [
    "--iterations",
    "30",
    "--warmups",
    "3",
    "--edit",
    `${corpus}/mod0001.tsx`,
    "--edit-mode",
    "same-span-body",
  ],
  // The same edit as above, except that it shifts every span after it: the
  // expensive incremental path, where positions cannot be reused.
  structural: [
    "--iterations",
    "8",
    "--warmups",
    "2",
    "--edit",
    `${corpus}/mod0001.tsx`,
    "--edit-mode",
    "prefix",
  ],
};
const arguments_ = scenarios[scenario];
if (!arguments_) {
  process.stderr.write(
    `usage: run-codspeed-case.mjs <${Object.keys(scenarios).join("|")}>\n`,
  );
  process.exit(2);
}

const result = spawnSync(benchmark, [...common, ...arguments_], {
  cwd: repository,
  encoding: "utf8",
  maxBuffer: 64 * 1024 * 1024,
});
if (result.error) throw result.error;
if (result.status !== 0) {
  process.stderr.write(result.stderr);
  process.exit(result.status ?? 1);
}

const report = JSON.parse(result.stdout);
process.stdout.write(
  `${scenario}: first=${report.firstAnalysisNs}ns median=${report.medianNs}ns p95=${report.p95Ns}ns\n`,
);
