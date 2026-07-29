#!/usr/bin/env node

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
const scenarios = {
  fresh: ["--iterations", "1", "--warmups", "0"],
  cached: ["--iterations", "4000", "--warmups", "3"],
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
