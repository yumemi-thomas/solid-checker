#!/usr/bin/env bun

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  cpSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
} from "node:fs";
import { homedir } from "node:os";
import { dirname, join, relative, resolve } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const CLI = join(ROOT, "packages/cli/bin/solid-checker.mjs");
const DEFAULT_NATIVE = join(ROOT, "rust/target/debug/solid-checker-rust");
const DEFAULT_TYPE_FACTS = join(ROOT, "bin/solid-typefacts");
const FIXTURE_ROOT = join(
  ROOT,
  "rust/target/contract-performance/solid-recharts-1.0.1/project-copy"
);

// Exact artifacts already populated by the authoritative ecosystem run. The
// harness never asks a registry to resolve a range and never modifies a
// checked-in node_modules tree. Each link points at immutable package bytes in
// Bun's content cache; changing this list is an explicit fixture update.
const CACHED_PACKAGES = {
  "solid-recharts": "solid-recharts@1.0.1@@@1",
  "solid-js": "solid-js@1.9.14@@@1",
  "csstype": "csstype@3.1.3@@@1",
  "seroval": "seroval@1.5.4@@@1",
  "seroval-plugins": "seroval-plugins@1.5.4@@@1",
  "d3-scale": "d3-scale@4.0.2@@@1",
  "d3-shape": "d3-shape@3.2.0@@@1",
  "d3-array": "d3-array@3.2.4@@@1",
  "d3-format": "d3-format@3.1.2@@@1",
  "d3-interpolate": "d3-interpolate@3.0.1@@@1",
  "d3-time": "d3-time@3.1.0@@@1",
  "d3-time-format": "d3-time-format@4.1.0@@@1",
  "d3-path": "d3-path@3.1.0@@@1",
  "d3-color": "d3-color@3.1.0@@@1",
  "internmap": "internmap@2.0.3@@@1",
  "@types/d3-scale": "@types/d3-scale@4.0.9@@@1",
  "@types/d3-shape": "@types/d3-shape@3.2.0@@@1",
  "@types/d3-time": "@types/d3-time@3.0.4@@@1",
  "@types/d3-path": "@types/d3-path@3.1.1@@@1"
};

function parseArguments(arguments_) {
  const options = { samples: 1, maxTotalMs: undefined, keep: false };
  for (let index = 0; index < arguments_.length; index += 1) {
    const argument = arguments_[index];
    if (argument === "--samples") options.samples = Number(arguments_[++index]);
    else if (argument === "--max-total-ms") options.maxTotalMs = Number(arguments_[++index]);
    else if (argument === "--keep") options.keep = true;
    else throw new Error(`unknown argument ${argument}`);
  }
  if (!Number.isInteger(options.samples) || options.samples <= 0) {
    throw new Error("--samples requires a positive integer");
  }
  if (options.maxTotalMs !== undefined && !(options.maxTotalMs > 0)) {
    throw new Error("--max-total-ms requires a positive number");
  }
  return options;
}

function copyPackage(cacheRoot, fixtureRoot, packageName, cacheEntry) {
  const source = join(cacheRoot, ...cacheEntry.split("/"));
  assert.ok(existsSync(join(source, "package.json")), `missing audited Bun cache artifact ${source}`);
  const destination = join(fixtureRoot, "node_modules", ...packageName.split("/"));
  mkdirSync(dirname(destination), { recursive: true });
  if (!existsSync(destination)) cpSync(source, destination, { recursive: true });
  const installed = readJson(join(destination, "package.json"));
  const cached = readJson(join(source, "package.json"));
  assert.deepEqual(
    { name: installed.name, version: installed.version },
    { name: cached.name, version: cached.version }
  );
}

function prepareFixture() {
  const cacheRoot =
    process.env.BUN_INSTALL_CACHE_DIR ?? join(process.env.BUN_INSTALL ?? join(homedir(), ".bun"), "install/cache");
  mkdirSync(FIXTURE_ROOT, { recursive: true });
  for (const [packageName, cacheEntry] of Object.entries(CACHED_PACKAGES)) {
    copyPackage(cacheRoot, FIXTURE_ROOT, packageName, cacheEntry);
  }
  return join(FIXTURE_ROOT, "node_modules/solid-recharts");
}

function run(command, args, { cwd, env, timeout }) {
  const started = performance.now();
  const result = spawnSync(command, args, {
    cwd,
    env,
    encoding: "utf8",
    timeout,
    maxBuffer: 128 * 1024 * 1024
  });
  return { ...result, wallMs: Math.round(performance.now() - started) };
}

function timingRecord(stderr) {
  for (const line of String(stderr).split(/\r?\n/)) {
    try {
      const parsed = JSON.parse(line);
      if (parsed.contractGenerationTiming) return parsed.contractGenerationTiming;
    } catch {
      // The timing channel coexists with human diagnostics. Only JSON records
      // addressed to this harness participate in the measurement.
    }
  }
  return undefined;
}

function readJson(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

function proposalStats(contract, plan) {
  assert.equal(contract.format, "solid-reactivity-contract");
  assert.equal(contract.schemaVersion, 2);
  assert.equal(contract.semanticModelVersion, 1);
  assert.equal(plan.format, "solid-checker-contract-proposal-plan");
  assert.equal(plan.planVersion, 1);
  let exports = 0;
  let artifactCases = 0;
  for (const entrypoint of Object.values(contract.entrypoints ?? {})) {
    const cases = Array.isArray(entrypoint.cases) ? entrypoint.cases : [entrypoint];
    for (const artifactCase of cases) {
      artifactCases += 1;
      exports += Object.keys(artifactCase.exports ?? {}).length;
    }
  }
  return { exports, artifactCases, closureCandidates: plan.closureCandidates?.length ?? 0 };
}

function assertGenerationShape(contract, plan, timing, directory) {
  const stats = proposalStats(contract, plan);
  assert.equal(contract.package.name, "solid-recharts");
  assert.equal(contract.package.version, "1.0.1");
  assert.equal(stats.exports, 109, `generated artifacts kept for inspection at ${directory}`);
  assert.ok(stats.closureCandidates > 0, "proposal plan carried no local closure candidates");
  assert.ok(timing, "generation emitted no structured timing record");
  assert.ok(timing.targets.length >= 2);
  return stats;
}

function sample(packageRoot, index, env, keep) {
  const runsRoot = join(ROOT, "rust/target/contract-performance/runs");
  mkdirSync(runsRoot, { recursive: true });
  const directory = mkdtempSync(join(runsRoot, `sample-${index}-`));
  const contractFile = join(directory, "solid-reactivity.json");
  let completed = false;
  try {
    const generated = run(
      process.execPath,
      [CLI, "contract", "generate", "--package-root", packageRoot, "--output", contractFile],
      { cwd: ROOT, env, timeout: 180_000 }
    );
    assert.equal(
      generated.status,
      0,
      `${generated.error?.message ?? ""}\n${generated.signal ?? ""}\n${generated.stderr}\n${generated.stdout}`
    );
    const contract = readJson(contractFile);
    const plan = readJson(`${contractFile}.proposal.json`);
    const timing = timingRecord(generated.stderr);
    const proposal = assertGenerationShape(contract, plan, timing, directory);
    completed = true;
    return {
      sample: index,
      generateMs: generated.wallMs,
      totalMs: generated.wallMs,
      generation: timing,
      proposal,
      outcome: "proposal",
      artifacts: keep ? relative(ROOT, directory) : undefined
    };
  } finally {
    if (!keep && completed) rmSync(directory, { recursive: true, force: true });
  }
}

const options = parseArguments(process.argv.slice(2));
for (const binary of [
  process.env.SOLID_CHECKER_NATIVE_BIN ?? DEFAULT_NATIVE,
  process.env.SOLID_TYPEFACTS_BIN ?? DEFAULT_TYPE_FACTS
]) {
  assert.ok(existsSync(binary), `required binary is absent: ${binary}`);
}
const env = {
  ...process.env,
  SOLID_CHECKER_NATIVE_BIN: process.env.SOLID_CHECKER_NATIVE_BIN ?? DEFAULT_NATIVE,
  SOLID_TYPEFACTS_BIN: process.env.SOLID_TYPEFACTS_BIN ?? DEFAULT_TYPE_FACTS,
  SOLID_CHECKER_TIMINGS: "1"
};
const packageRoot = prepareFixture();
const samples = Array.from({ length: options.samples }, (_, index) =>
  sample(packageRoot, index + 1, env, options.keep)
);
const totals = samples.map(result => result.totalMs).toSorted((left, right) => left - right);
const medianTotalMs = totals[Math.floor(totals.length / 2)];
const report = { fixture: relative(ROOT, packageRoot), samples, medianTotalMs };
process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
if (options.maxTotalMs !== undefined) {
  assert.ok(
    medianTotalMs <= options.maxTotalMs,
    `median ${medianTotalMs}ms exceeds ${options.maxTotalMs}ms performance limit`
  );
}
