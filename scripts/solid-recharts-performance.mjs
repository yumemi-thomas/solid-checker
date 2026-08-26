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

import { expandContract } from "../packages/cli/scripts/contract-document.mjs";
import { buildProbePlan } from "../packages/cli/scripts/contract-probe-driver.mjs";

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

function sibling(path, suffix) {
  return path.endsWith(".json") ? `${path.slice(0, -5)}${suffix}` : `${path}${suffix}`;
}

function readJson(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

function assertSlowShape({ contract, timing, probe, verify, verifyResult }) {
  const expanded = expandContract(contract);
  assert.equal(contract.package.name, "solid-recharts");
  assert.equal(contract.package.version, "1.0.1");
  assert.equal(Object.keys(expanded.entrypoints["."].exports).length, 109);
  assert.equal(buildProbePlan(expanded).claims.length, 140);

  assert.ok(timing, "generation emitted no structured timing record");
  assert.ok(timing.targets.length >= 2);
  assert.equal(new Set(timing.targets.map(target => target.target)).size, 2);
  const targets = timing.targets.toSorted((left, right) => left.conditions.join().localeCompare(right.conditions.join()));
  assert.ok(targets.some(target => target.conditions.includes("browser")));
  assert.ok(targets.some(target => target.conditions.includes("node")));
  for (const target of targets) {
    assert.ok(target.runtimeFiles >= 200 && target.runtimeFiles <= 300, JSON.stringify(target));
    assert.ok(target.runtimeBytes >= 700_000 && target.runtimeBytes <= 1_000_000, JSON.stringify(target));
  }

  assert.equal(probe.summary.claims, 140);
  assert.equal(probe.summary.passed + probe.summary.failed + probe.summary.undriven, 140);
  assert.equal(probe.summary.failed, 0);
  assert.deepEqual(probe.modes, ["client", "server", "development", "production"]);
  assert.ok(probe.sessions.chains >= 4, JSON.stringify(probe.sessions));
  assert.ok(probe.sessions.started >= 100, JSON.stringify(probe.sessions));
  assert.ok(probe.sessions.restarts >= 90, JSON.stringify(probe.sessions));
  assert.ok(Object.values(probe.sessions.byMode).every(mode => mode.chains >= 1));

  assert.equal(verifyResult.status, 1, verifyResult.stderr);
  assert.equal(verify.outcome, "refused");
  const blockers = (verify.blockers?.raised ?? []).join("\n");
  for (const name of ["Dot", "LabelList", "Pie"]) assert.match(blockers, new RegExp(`\\b${name}\\b`));
  assert.match(blockers, /kind/);
}

function assertGenerationShape(contract, timing, directory) {
  const expanded = expandContract(contract);
  assert.equal(contract.package.name, "solid-recharts");
  assert.equal(contract.package.version, "1.0.1");
  assert.equal(
    Object.keys(expanded.entrypoints["."].exports).length,
    109,
    `generated artifacts kept for inspection at ${directory}`
  );
  assert.equal(buildProbePlan(expanded).claims.length, 140);
  assert.ok(timing, "generation emitted no structured timing record");
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
    const timing = timingRecord(generated.stderr);
    assertGenerationShape(contract, timing, directory);

    const probed = run(
      process.execPath,
      [
        CLI,
        "contract",
        "probe",
        contractFile,
        "--package-root",
        packageRoot,
        "--timeout",
        "160000"
      ],
      { cwd: ROOT, env, timeout: 160_000 }
    );
    assert.equal(probed.status, 0, probed.stderr);
    const probe = readJson(sibling(contractFile, ".probe.json"));

    const verified = run(
      process.execPath,
      [CLI, "contract", "verify", contractFile],
      { cwd: ROOT, env, timeout: 30_000 }
    );
    const verify = readJson(sibling(contractFile, ".verify.json"));
    assertSlowShape({ contract, timing, probe, verify, verifyResult: verified });

    const result = {
      sample: index,
      generateMs: generated.wallMs,
      probeMs: probed.wallMs,
      verifyMs: verified.wallMs,
      totalMs: generated.wallMs + probed.wallMs + verified.wallMs,
      generation: timing,
      claims: probe.summary,
      sessions: probe.sessions,
      outcome: verify.outcome,
      artifacts: keep ? relative(ROOT, directory) : undefined
    };
    completed = true;
    return result;
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
