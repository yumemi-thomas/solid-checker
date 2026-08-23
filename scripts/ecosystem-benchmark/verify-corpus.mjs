#!/usr/bin/env node
// Machine-verification harness for the pinned ecosystem corpus: how many real
// packages verify end to end.
//
// For every manifest probe row it runs the full RFC 0002 pipeline against a
// throwaway install:
//
//   npm install --ignore-scripts  ->  contract generate
//     ->  contract probe --write  ->  contract verify
//
// and writes benchmarks/ecosystem/verification-report.{json,md}.
//
// Three things matter more here than convenience:
//
// - **`contract probe` executes package code.** That is the whole reason this
//   is a separate command from `contract generate`, whose stated design
//   property is that it imports nothing. Every install and every execution
//   here happens inside a temporary directory under the state directory, npm
//   runs with `--ignore-scripts` so no package lifecycle script ever executes,
//   and every probe runs under both a per-mode child timeout and a whole-phase
//   wall budget. Run it where you would run those packages' own test suites.
// - **It is deliberately not `run.mjs`.** That harness's checked-in reports
//   measure contract *generation*, and folding a verification measurement into
//   them would change what its numbers mean. This one only reads that
//   harness's manifest, install helpers, and failure classifier.
// - **A timeout is its own outcome, never a verification result.** A row whose
//   probe exceeded the wall budget is recorded as `probe-timeout` and is
//   counted as neither verified nor refused. Silently folding it either way
//   would be the one wrong answer this measurement could give.
//
// The run is resumable: every completed row is appended to a journal in the
// state directory, and a re-run skips what the journal already records. Pass
// `--aggregate-only` to rebuild the reports from an existing journal without
// re-running anything.

import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { appendFileSync, existsSync, mkdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import { classifyResult } from "./lib/classify.mjs";
import { FAMILIES } from "./lib/families.mjs";
import {
  createProject,
  installPackages,
  readInstalledVersions,
  readLockIntegrity,
  verifyInstall
} from "./lib/install.mjs";
import { sortRows } from "./lib/manifest.mjs";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const MANIFEST = join(ROOT, "scripts/ecosystem-benchmark/manifest.json");
const CLI = join(ROOT, "packages/cli/bin/solid-checker.mjs");
const REPORT_DIR = join(ROOT, "benchmarks/ecosystem");

const DEFAULTS = {
  installTimeoutMs: 240_000,
  generateTimeoutMs: 120_000,
  // Per condition-mode child process. The driver restarts a worker after every
  // probe that threw, so this bounds one attempt, not the mode.
  probeModeTimeoutMs: 20_000,
  // The whole `contract probe` invocation, restarts included.
  probeWallBudgetMs: 120_000,
  verifyTimeoutMs: 90_000,
  concurrency: 6
};

// ---------------------------------------------------------------------------
// Pure helpers (exported for direct unit testing).
// ---------------------------------------------------------------------------

/// The CLI's own sibling-path rule (contract-review-plan.mjs): the probe and
/// verify sidecars *replace* a trailing `.json`, they do not append to it.
export function siblingPath(output, suffix) {
  return output.toLowerCase().endsWith(".json") ? `${output.slice(0, -5)}${suffix}` : `${output}${suffix}`;
}

/// The blocker taxonomy of RFC 0002 §3 (`BLOCKERS` in
/// packages/cli/scripts/contract-verification.mjs), recovered from the refusal
/// text.
///
/// `contract verify` writes no sidecar when it refuses -- the report it builds
/// exists only on the success path, and its `blockers.raised` is always `[]` --
/// so the stderr line is the only record of what was raised. The refusal text
/// embeds an absolute contract path, so a distinguishing clause can sit far
/// into the line; each rule below matches on the earliest marker that is
/// unambiguous.
export function blockerClass(line) {
  if (line.startsWith("no probe report at")) return "probe-report-present";
  if (/records \d+ passed claim/.test(line)) return "probe-report-includes-evidence-write";
  if (line.includes("but no evidence write")) return "probe-report-includes-evidence-write";
  if (line.includes("was written for contract bytes")) return "probe-report-binds-contract";
  if (line.includes("re-probe these exact bytes")) return "probe-report-binds-contract";
  if (line.includes("and the contract describes")) return "probe-report-binds-contract";
  if (line.includes("regenerate the contract, re-probe it")) return "probe-report-binds-contract";
  if (line.includes("discovery")) return "probe-report-includes-discovery";
  if (line.startsWith("a probe failed")) return "probe-failed";
  if (line.startsWith("an incompleteness finding")) return "incompleteness";
  if (line.includes("carries a closure note")) return "closure-note";
  if (line.includes("no passing kind observation")) return "kind-observed";
  if (line.includes("review decision") || line.includes("a promotion to")) return "review-under-way";
  if (line.includes("does not validate")) return "document-validates";
  return "unclassified-refusal";
}

/// One root cause per refusal, in the order a reader has to resolve them.
///
/// `probe-report-includes-evidence-write` is deliberately last: it is raised
/// *because* `contract probe --write` declined to write after a failure or an
/// incompleteness finding, so on almost every row it is a consequence of
/// another blocker rather than an independent cause. A row where it stands
/// alone is a different and worth-naming shape, which is why it stays in the
/// list rather than being dropped.
export const ROOT_CAUSE_ORDER = [
  "probe-failed",
  "incompleteness",
  "kind-observed",
  "closure-note",
  "probe-report-binds-contract",
  "probe-report-includes-discovery",
  "document-validates",
  "review-under-way",
  "probe-report-present",
  "probe-report-includes-evidence-write",
  "unclassified-refusal"
];

export function rootCause(classes) {
  for (const name of ROOT_CAUSE_ORDER) if (classes.has(name)) return name;
  return "unclassified-refusal";
}

/// Why a claim went undriven. The driver's reasons are free text carrying
/// package-specific detail -- a thrown message, an absolute path -- so grouping
/// strips the detail to make a distribution readable. The raw reason survives
/// per row in the journal and in the JSON report.
export function undrivenBucket(reason) {
  const text = String(reason);
  if (text.startsWith("reactive reads are proven from compiler facts")) return "no probe form: reactiveReads";
  if (text.startsWith("owner requirements are proven")) return "no probe form: ownerRequirements";
  if (text.startsWith("an identity claim about a parameter")) return "no probe form: parameter identity";
  if (text.startsWith("callback argument descriptors have no probe form"))
    return "no probe form: callback arguments";
  if (text.startsWith("writeProbeEvidence does not descend into return leaves"))
    return "no probe form: nested return leaf";
  if (text.startsWith("asyncBehavior has no")) return "no probe form: asyncBehavior";
  if (text.startsWith("no generic store-path observation")) return "no probe form: store path";
  if (text.startsWith("no plantable reactive source")) return "no plantable reactive source";
  if (text.startsWith("the synthesized call completed without invoking the callback"))
    return "synthesized call did not invoke the callback";
  if (text.startsWith("the synthesized call threw")) return "synthesized call threw";
  if (text.startsWith("import of ")) return "entrypoint import threw";
  if (text.startsWith("the probe process exited") || text.startsWith("the probe process was killed"))
    return "probe session failed (process died)";
  if (/^spawnSync .*ETIMEDOUT/.test(text)) return "probe session hit the per-mode timeout";
  if (text.includes("no re-read followed the planted write")) return "planted write was never re-read";
  if (text.startsWith("the callback ran only once the returned accessor was read"))
    return "callback ownership ambiguous in the driver's read scope";
  if (text.startsWith("the probe process stopped before reaching this claim"))
    return "probe session stopped before this claim";
  if (text.startsWith("the probe process wrote no readable report")) return "probe session wrote no report";
  if (text.startsWith("no unambiguous summary")) return "no unambiguous summary for the mode";
  return "other";
}

export function probeErrorBucket(detail) {
  const text = String(detail ?? "");
  if (text.includes("no installed solid-js above")) return "no installed solid-js beside the package";
  if (text.includes("names no dialect this checker probes")) return "solid-js version names no dialect";
  if (text.includes("cannot find an installed")) return "package root not resolvable";
  if (text.includes("regenerate the contract for the installed release"))
    return "installed release differs from the contract";
  return "other";
}

/// Every export in a contract document, split into the two states a verified
/// document can leave one in: a claim the machine stands behind, and the honest
/// `{"status":"unknown"}` sentinel.
///
/// `expandContract` is required rather than optional: a document dedups
/// summaries into a `summaries` table and maps summary-id -> export *names*, so
/// counting off the raw document would count summary ids. A document that
/// cannot be expanded records the error rather than a row of zeroes.
export function classifyExports(rawDocument, expandContract) {
  const result = { exports: 0, unknownBearing: 0, entrypoints: 0, expandError: null };
  let document = null;
  try {
    document = expandContract(rawDocument);
  } catch (error) {
    result.expandError = String(error?.message ?? error).slice(0, 200);
    return result;
  }
  if (!document?.entrypoints) return result;
  const bearsUnknown = value => {
    if (Array.isArray(value)) return value.some(bearsUnknown);
    if (value && typeof value === "object") {
      if (value.status === "unknown" && Object.keys(value).length === 1) return true;
      return Object.values(value).some(bearsUnknown);
    }
    return false;
  };
  for (const entry of Object.values(document.entrypoints)) {
    result.entrypoints += 1;
    for (const summary of Object.values(entry.exports ?? {})) {
      result.exports += 1;
      if (typeof summary === "object" && bearsUnknown(summary)) result.unknownBearing += 1;
    }
  }
  return result;
}

export function notVerifiedLines(stderr) {
  return String(stderr)
    .split("\n")
    .filter(line => line.includes("solid-checker: not verified:"))
    .map(line => line.slice(line.indexOf("not verified:") + "not verified:".length).trim());
}

export function percentile(values, fraction) {
  if (!values.length) return null;
  const sorted = [...values].sort((left, right) => left - right);
  return sorted[Math.min(sorted.length - 1, Math.floor(fraction * (sorted.length - 1)))];
}

export function stats(values) {
  if (!values.length) return { count: 0, medianMs: null, p90Ms: null, maxMs: null, meanMs: null };
  const total = values.reduce((sum, value) => sum + value, 0);
  return {
    count: values.length,
    medianMs: percentile(values, 0.5),
    p90Ms: percentile(values, 0.9),
    maxMs: Math.max(...values),
    meanMs: Math.round(total / values.length)
  };
}

// ---------------------------------------------------------------------------
// Running one row
// ---------------------------------------------------------------------------

function run(command, args, { cwd, timeoutMs, env } = {}) {
  return new Promise(resolvePromise => {
    const child = spawn(command, args, {
      cwd,
      env: { ...process.env, ...env },
      stdio: ["ignore", "pipe", "pipe"]
    });
    let stdout = "";
    let stderr = "";
    let timedOut = false;
    let spawnError = "";
    child.on("error", error => {
      spawnError = `${error.code ?? "spawn error"}: ${error.message}`;
    });
    const timer = timeoutMs
      ? setTimeout(() => {
          timedOut = true;
          child.kill("SIGKILL");
        }, timeoutMs)
      : null;
    child.stdout.on("data", chunk => {
      stdout += chunk;
    });
    child.stderr.on("data", chunk => {
      stderr += chunk;
    });
    child.on("close", status => {
      if (timer) clearTimeout(timer);
      resolvePromise({
        status,
        stdout,
        stderr: spawnError ? `${stderr}\n${spawnError}` : stderr,
        timedOut
      });
    });
  });
}

function readJson(path) {
  try {
    return JSON.parse(readFileSync(path, "utf8"));
  } catch {
    return null;
  }
}

async function runRow({ row, probe }, context) {
  const { workDir, budgets, cliEnv, expandContract } = context;
  const started = Date.now();
  const record = {
    probeId: probe.id,
    package: row.package,
    version: row.version,
    family: row.family,
    solidTarget: row.solidTarget,
    probeKind: probe.kind,
    channel: probe.channel,
    solid: probe.solid,
    startedAt: new Date(started).toISOString()
  };

  const projectDir = await mkdtemp(join(workDir, "proj-"));
  const outputDir = await mkdtemp(join(workDir, "out-"));
  try {
    const specs = [
      `${row.package}@${row.version}`,
      ...Object.entries(probe.solid ?? {}).map(([name, version]) => `${name}@${version}`)
    ];
    const expected = {
      [row.package]: { version: row.version, integrity: row.integrity ?? null },
      ...Object.fromEntries(
        Object.entries(probe.solid ?? {}).map(([name, version]) => [name, { version, integrity: null }])
      )
    };

    const installStart = Date.now();
    await createProject({ root: projectDir, specs });
    const installResult = await installPackages({
      projectDir,
      specs,
      timeoutMs: budgets.installTimeoutMs
    });
    const installedVersions = readInstalledVersions(projectDir, Object.keys(expected));
    const integrity = readLockIntegrity(projectDir, Object.keys(expected));
    record.installMs = Date.now() - installStart;

    const installClass = classifyResult({
      status: installResult.status,
      stdout: installResult.stdout,
      stderr: installResult.stderr,
      timedOut: installResult.timedOut,
      phase: "install"
    });
    const verified =
      installClass.class === "success"
        ? verifyInstall({ expected, versions: installedVersions, integrity })
        : { ok: true, problems: [] };
    if (installClass.class !== "success" || !verified.ok) {
      record.stage = "install";
      record.outcome = "install-failure";
      record.installClass = installClass.class;
      record.detail = verified.ok
        ? installClass.signature
        : verified.problems.map(problem => `${problem.kind}:${problem.package}`).join("; ");
      record.totalMs = Date.now() - started;
      return record;
    }

    const packageRoot = join(projectDir, "node_modules", ...row.package.split("/"));
    // The contract is written OUTSIDE the install tree, for the same reason
    // run.mjs does it: a report artifact living inside node_modules could be
    // mistaken for package content by the package's own tooling.
    const contractFile = join(outputDir, "solid-reactivity.json");

    const generateStart = Date.now();
    const generateResult = await run(
      process.execPath,
      [CLI, "contract", "generate", "--package-root", packageRoot, "--output", contractFile],
      { timeoutMs: budgets.generateTimeoutMs, env: cliEnv }
    );
    record.generateMs = Date.now() - generateStart;
    const generateClass = classifyResult({
      status: generateResult.status,
      stdout: generateResult.stdout,
      stderr: generateResult.stderr,
      timedOut: generateResult.timedOut,
      phase: "generate"
    });
    record.generateClass = generateClass.class;
    record.refusedEntrypoints = generateClass.detail?.refusedEntrypoints ?? 0;
    if (generateClass.class !== "success" && generateClass.class !== "partial-success") {
      record.stage = "generate";
      record.outcome = "generate-failure";
      record.detail = generateClass.signature;
      record.totalMs = Date.now() - started;
      return record;
    }
    record.generated = classifyExports(readJson(contractFile), expandContract);

    const probeStart = Date.now();
    const probeResult = await run(
      process.execPath,
      [
        CLI,
        "contract",
        "probe",
        contractFile,
        "--package-root",
        packageRoot,
        // Discovery -- planting a callback where the contract states none -- is
        // never disabled: it is the only automated check that can contradict a
        // negative claim, and `contract verify` refuses a report produced
        // without it outright.
        "--write",
        "--timeout",
        String(budgets.probeModeTimeoutMs)
      ],
      { timeoutMs: budgets.probeWallBudgetMs, env: cliEnv }
    );
    record.probeMs = Date.now() - probeStart;
    record.probeExit = probeResult.status;
    record.probeTimedOut = probeResult.timedOut;
    if (probeResult.timedOut) {
      record.stage = "probe";
      record.outcome = "probe-timeout";
      record.detail = `probe exceeded the ${budgets.probeWallBudgetMs}ms wall budget`;
      record.totalMs = Date.now() - started;
      return record;
    }

    const probeReport = readJson(siblingPath(contractFile, ".probe.json"));
    if (!probeReport) {
      record.stage = "probe";
      record.outcome = "probe-error";
      record.detail = (probeResult.stderr || probeResult.stdout).slice(0, 600).trim();
      record.totalMs = Date.now() - started;
      return record;
    }
    record.probe = {
      summary: probeReport.summary,
      modes: probeReport.modes,
      discovery: {
        enabled: probeReport.discovery?.enabled ?? false,
        parameters: (probeReport.discovery?.parameters ?? []).length
      },
      dialect: probeReport.identities?.dialect ?? null,
      runtime: probeReport.identities?.runtime ?? null,
      markersWritten: probeReport.contract?.markersWritten ?? 0,
      markersSuperseded: probeReport.contract?.markersSuperseded ?? 0,
      wrote: probeReport.contract?.afterWrite !== undefined,
      incompleteness: (probeReport.incompleteness ?? []).map(finding => finding.text).slice(0, 20),
      stderrTail: probeResult.stderr.slice(-1500)
    };
    const undrivenReasons = {};
    const failedClaims = [];
    const claimFamilies = {};
    for (const claim of probeReport.claims ?? []) {
      const key = `${claim.family}:${claim.status}`;
      claimFamilies[key] = (claimFamilies[key] ?? 0) + 1;
      if (claim.status === "undriven") {
        const reason = claim.reason ?? "(no reason recorded)";
        undrivenReasons[reason] = (undrivenReasons[reason] ?? 0) + 1;
      } else if (claim.status === "failed") {
        failedClaims.push(`${claim.entrypoint}:${claim.export} ${claim.claim}: ${claim.reason}`);
      }
    }
    record.probe.undrivenReasons = undrivenReasons;
    record.probe.claimFamilies = claimFamilies;
    record.probe.failedClaims = failedClaims.slice(0, 10);

    const verifyStart = Date.now();
    const verifyResult = await run(process.execPath, [CLI, "contract", "verify", contractFile], {
      timeoutMs: budgets.verifyTimeoutMs,
      env: cliEnv
    });
    record.verifyMs = Date.now() - verifyStart;
    record.verifyExit = verifyResult.status;
    record.verifyTimedOut = verifyResult.timedOut;

    const verifyReport = readJson(siblingPath(contractFile, ".verify.json"));
    record.final = classifyExports(readJson(contractFile), expandContract);

    if (verifyResult.timedOut) {
      record.stage = "verify";
      record.outcome = "verify-timeout";
    } else if (verifyReport && readJson(contractFile)?.evidence?.kind === "verified") {
      record.stage = "verify";
      record.outcome = "verified";
      record.verify = {
        summary: verifyReport.summary,
        conversions: (verifyReport.conversions ?? []).map(conversion => ({
          entrypoint: conversion.entrypoint,
          export: conversion.export,
          field: conversion.field,
          reason: conversion.claims?.[0]?.reason ?? null
        }))
      };
    } else {
      record.stage = "verify";
      record.outcome = "refused";
      const lines = notVerifiedLines(verifyResult.stderr);
      // Every refusal line is kept -- truncated to a head long enough to
      // classify -- because a capped list could hide a blocker class entirely.
      record.blockerCount = lines.length;
      record.blockerHeads = lines.slice(0, 400).map(line => line.slice(0, 260));
      record.blockers = lines.slice(0, 5);
      if (!lines.length) {
        record.detail = (verifyResult.stderr || verifyResult.stdout).slice(0, 600).trim();
        record.outcome = "verify-error";
      }
    }
    record.totalMs = Date.now() - started;
    return record;
  } catch (error) {
    record.stage = record.stage ?? "harness";
    record.outcome = "harness-error";
    record.detail = String(error?.stack ?? error).slice(0, 600);
    record.totalMs = Date.now() - started;
    return record;
  } finally {
    await rm(projectDir, { recursive: true, force: true }).catch(() => {});
    await rm(outputDir, { recursive: true, force: true }).catch(() => {});
  }
}

// ---------------------------------------------------------------------------
// Aggregation
// ---------------------------------------------------------------------------

function emptyAggregate() {
  return {
    rows: 0,
    outcomes: {},
    contractsGenerated: 0,
    verified: 0,
    refused: 0,
    claims: { total: 0, driven: 0, passed: 0, failed: 0, undriven: 0, incompleteness: 0 },
    undriven: {},
    blockerRows: {},
    blockerLines: {},
    rootCauses: {},
    conversions: 0,
    conversionFields: {},
    probedRowsKept: 0,
    rowsWithProbedEvidence: 0,
    droppedInferredMarkers: 0,
    staleProbedMarkers: 0,
    exports: {
      certifiedInVerified: 0,
      unknownInVerified: 0,
      inUnverifiedContract: 0,
      draftUnknownInVerified: 0,
      draftExportsInVerified: 0
    },
    totals: []
  };
}

function accumulate(bucket, record) {
  if (!bucket) return;
  bucket.rows += 1;
  bucket.outcomes[record.outcome] = (bucket.outcomes[record.outcome] ?? 0) + 1;
  bucket.totals.push(record.totalMs);
  if (record.generated) bucket.contractsGenerated += 1;
  if (record.probe?.summary) {
    const summary = record.probe.summary;
    bucket.claims.total += summary.claims ?? 0;
    bucket.claims.driven += summary.driven ?? 0;
    bucket.claims.passed += summary.passed ?? 0;
    bucket.claims.failed += summary.failed ?? 0;
    bucket.claims.undriven += summary.undriven ?? 0;
    bucket.claims.incompleteness += summary.incompleteness ?? 0;
    for (const [reason, count] of Object.entries(record.probe.undrivenReasons ?? {})) {
      const key = undrivenBucket(reason);
      bucket.undriven[key] = (bucket.undriven[key] ?? 0) + count;
    }
  }
  if (record.outcome === "verified") {
    bucket.verified += 1;
    bucket.conversions += record.verify?.summary?.conversions ?? 0;
    bucket.probedRowsKept += record.verify?.summary?.probedRows ?? 0;
    if ((record.verify?.summary?.probedRows ?? 0) > 0) bucket.rowsWithProbedEvidence += 1;
    bucket.droppedInferredMarkers += record.verify?.summary?.droppedInferredMarkers ?? 0;
    bucket.staleProbedMarkers += record.verify?.summary?.staleProbedMarkers ?? 0;
    for (const conversion of record.verify?.conversions ?? []) {
      const field = conversion.field.split(".").pop();
      bucket.conversionFields[field] = (bucket.conversionFields[field] ?? 0) + 1;
    }
    bucket.exports.certifiedInVerified += (record.final?.exports ?? 0) - (record.final?.unknownBearing ?? 0);
    bucket.exports.unknownInVerified += record.final?.unknownBearing ?? 0;
    bucket.exports.draftUnknownInVerified += record.generated?.unknownBearing ?? 0;
    bucket.exports.draftExportsInVerified += record.generated?.exports ?? 0;
  } else if (record.generated) {
    bucket.exports.inUnverifiedContract += record.final?.exports ?? record.generated?.exports ?? 0;
  }
  if (record.outcome === "refused" || record.outcome === "verify-error") {
    bucket.refused += 1;
    const classes = new Set((record.blockerHeads ?? []).map(blockerClass));
    for (const name of classes) bucket.blockerRows[name] = (bucket.blockerRows[name] ?? 0) + 1;
    for (const line of record.blockerHeads ?? []) {
      const name = blockerClass(line);
      bucket.blockerLines[name] = (bucket.blockerLines[name] ?? 0) + 1;
    }
    const cause = rootCause(classes);
    bucket.rootCauses[cause] = (bucket.rootCauses[cause] ?? 0) + 1;
  }
}

function finish(bucket) {
  const { totals, ...rest } = bucket;
  return { ...rest, wallMs: stats(totals) };
}

export function buildVerificationReport({ records, manifest, budgets, checker }) {
  const overall = emptyAggregate();
  const byFamily = new Map(FAMILIES.map(family => [family.id, emptyAggregate()]));
  const bySolidTarget = new Map([
    ["solid1", emptyAggregate()],
    ["solid2", emptyAggregate()]
  ]);
  const probeErrors = {};
  const installFailures = [];
  const generateFailures = [];
  const timeouts = [];

  for (const record of records) {
    accumulate(overall, record);
    accumulate(byFamily.get(record.family), record);
    accumulate(bySolidTarget.get(record.solidTarget), record);
    if (record.outcome === "probe-error") {
      const bucket = probeErrorBucket(record.detail);
      probeErrors[bucket] = (probeErrors[bucket] ?? 0) + 1;
    }
    if (record.outcome === "install-failure") installFailures.push(record.probeId);
    if (record.outcome === "generate-failure")
      generateFailures.push({ probeId: record.probeId, class: record.generateClass });
    if (record.outcome === "probe-timeout" || record.outcome === "verify-timeout")
      timeouts.push({ probeId: record.probeId, outcome: record.outcome, totalMs: record.totalMs });
  }

  // Why an entrypoint could not be imported at all -- the single largest
  // undriven cause, and a fact about the environment the probe worker runs in
  // as much as about the package.
  const importThrows = {};
  const importThrowRows = new Set();
  for (const record of records) {
    for (const [reason, count] of Object.entries(record.probe?.undrivenReasons ?? {})) {
      if (!reason.startsWith("import of ")) continue;
      importThrowRows.add(record.probeId);
      const match = /threw: ([A-Za-z]+(?: \[[A-Z_]+\])?): ?(.*)/.exec(reason);
      const key = (match ? `${match[1]}: ${match[2].slice(0, 70)}` : reason.slice(0, 80)).replace(
        /\/(?:private\/)?tmp\/\S+/g,
        "<path>"
      );
      importThrows[key] = (importThrows[key] ?? 0) + count;
    }
  }

  const phaseWallMs = {
    install: stats(records.map(record => record.installMs).filter(Number.isFinite)),
    generate: stats(records.map(record => record.generateMs).filter(Number.isFinite)),
    probe: stats(records.map(record => record.probeMs).filter(Number.isFinite)),
    verify: stats(records.map(record => record.verifyMs).filter(Number.isFinite)),
    pipelineWithoutInstall: stats(
      records
        .map(record =>
          Number.isFinite(record.generateMs)
            ? (record.generateMs ?? 0) + (record.probeMs ?? 0) + (record.verifyMs ?? 0)
            : null
        )
        .filter(Number.isFinite)
    ),
    total: stats(records.map(record => record.totalMs).filter(Number.isFinite))
  };

  const startedAt = records.map(record => record.startedAt).filter(Boolean).sort()[0] ?? null;
  const finishedAt = records.map(record => record.finishedAt).filter(Boolean).sort().at(-1) ?? null;

  const refusals = records
    .filter(record => record.outcome === "refused" || record.outcome === "verify-error")
    .map(record => {
      const classes = new Set((record.blockerHeads ?? []).map(blockerClass));
      return {
        probeId: record.probeId,
        package: record.package,
        version: record.version,
        family: record.family,
        solidTarget: record.solidTarget,
        blockerCount: record.blockerCount ?? 0,
        blockerClasses: [...classes].sort(),
        rootCause: rootCause(classes),
        firstBlocker: record.blockers?.[0] ?? record.detail ?? null,
        claims: record.probe?.summary ?? null,
        exports: record.final?.exports ?? null
      };
    })
    .sort((left, right) => left.probeId.localeCompare(right.probeId));

  const verified = records
    .filter(record => record.outcome === "verified")
    .map(record => ({
      probeId: record.probeId,
      package: record.package,
      version: record.version,
      family: record.family,
      solidTarget: record.solidTarget,
      exports: record.final?.exports ?? 0,
      exportsUnknown: record.final?.unknownBearing ?? 0,
      exportsUnknownAtGeneration: record.generated?.unknownBearing ?? 0,
      conversions: record.verify?.summary?.conversions ?? 0,
      probedRowsKept: record.verify?.summary?.probedRows ?? 0,
      claims: record.probe?.summary ?? null,
      totalMs: record.totalMs
    }))
    .sort((left, right) => left.probeId.localeCompare(right.probeId));

  return {
    schemaVersion: 1,
    kind: "ecosystem-machine-verification",
    aggregatedAt: new Date().toISOString(),
    startedAt,
    finishedAt,
    safety: {
      executesPackageCode: true,
      note:
        "`contract probe` imports and runs each installed package's code in child processes. Every " +
        "install and every execution in this run happened inside temporary directories under the " +
        "harness state directory, npm ran with --ignore-scripts so no package lifecycle script " +
        "executed, and each probe process had both a per-mode timeout and a whole-phase wall budget."
    },
    pipeline: [
      "npm install --ignore-scripts",
      "contract generate",
      "contract probe --write",
      "contract verify"
    ],
    budgets,
    checker,
    corpus: {
      manifest: "scripts/ecosystem-benchmark/manifest.json",
      manifestGeneratedAt: manifest.generatedAt,
      rows: manifest.rows.length,
      probesInManifest: manifest.rows.reduce((total, row) => total + (row.probes?.length ?? 0), 0),
      probesRun: records.length,
      scope: "full corpus (every official row's every probe; supplemental fork rows excluded)"
    },
    overall: finish(overall),
    byFamily: Object.fromEntries(
      FAMILIES.map(family => [family.id, { label: family.label, ...finish(byFamily.get(family.id)) }])
    ),
    bySolidTarget: Object.fromEntries([...bySolidTarget].map(([name, bucket]) => [name, finish(bucket)])),
    phaseWallMs,
    probeEnvironment: {
      rowsWithAnEntrypointImportThrow: importThrowRows.size,
      importThrows
    },
    preContractFailures: { installFailures, generateFailures, probeErrors, timeouts },
    refusals,
    verified
  };
}

export function renderVerificationMarkdown(report) {
  const rate = (numerator, denominator) =>
    denominator ? `${numerator}/${denominator} (${((numerator / denominator) * 100).toFixed(2)}%)` : "n/a";
  const sortedEntries = object => Object.entries(object).sort((left, right) => right[1] - left[1]);
  const overall = report.overall;
  const lines = [];

  lines.push("# Ecosystem machine-verification report");
  lines.push("");
  lines.push(
    "How many real ecosystem packages machine-verify end to end: `contract generate` -> " +
      "`contract probe --write` -> `contract verify`, run against a throwaway install of every probe " +
      "row in the pinned corpus."
  );
  lines.push("");
  lines.push("> **This measurement executes package code.** `contract probe` imports and runs each");
  lines.push("> installed package, and its dependencies, in child processes. Every install and every");
  lines.push("> execution happened inside temporary directories under the harness state directory, npm");
  lines.push("> ran with `--ignore-scripts` so no package lifecycle script executed, and each probe ran");
  lines.push("> under both a per-mode timeout and a whole-phase wall budget.");
  lines.push("");
  lines.push(`- Started: ${report.startedAt}`);
  lines.push(`- Finished: ${report.finishedAt}`);
  lines.push(
    `- Manifest generated at: ${report.corpus.manifestGeneratedAt} (rows: ${report.corpus.rows}, probes: ${report.corpus.probesInManifest})`
  );
  lines.push(`- Probe rows run: ${report.corpus.probesRun}`);
  lines.push(
    `- Checker native binary: \`${report.checker.nativeBin.sha256}\` (${report.checker.nativeBin.size} bytes, mtime ${report.checker.nativeBin.mtime})`
  );
  lines.push(
    `- Type Facts binary: \`${report.checker.typeFactsBin.sha256}\` (${report.checker.typeFactsBin.size} bytes, mtime ${report.checker.typeFactsBin.mtime})`
  );
  lines.push(
    `- Budgets: install ${report.budgets.installTimeoutMs} ms, generate ${report.budgets.generateTimeoutMs} ms, ` +
      `probe ${report.budgets.probeModeTimeoutMs} ms per condition mode / ${report.budgets.probeWallBudgetMs} ms whole phase, ` +
      `verify ${report.budgets.verifyTimeoutMs} ms; concurrency ${report.budgets.concurrency}`
  );
  lines.push("");

  lines.push("## Headline");
  lines.push("");
  lines.push("| Figure | Count |");
  lines.push("| --- | --- |");
  lines.push(`| Probe rows run | ${report.corpus.probesRun} |`);
  lines.push(`| Reached a generated contract | ${rate(overall.contractsGenerated, overall.rows)} |`);
  lines.push(`| **Reached \`verified\`** | **${rate(overall.verified, overall.rows)}** of all rows |`);
  lines.push(
    `| Reached \`verified\`, of rows that produced a contract | ${rate(overall.verified, overall.contractsGenerated)} |`
  );
  lines.push(`| Refused by \`contract verify\` | ${rate(overall.refused, overall.rows)} |`);
  lines.push("");
  lines.push("Outcome classes, raw:");
  lines.push("");
  lines.push("| Outcome | Rows |");
  lines.push("| --- | --- |");
  for (const [name, count] of sortedEntries(overall.outcomes)) lines.push(`| \`${name}\` | ${count} |`);
  lines.push("");

  lines.push("## Per family");
  lines.push("");
  lines.push(
    "| Family | Rows | Contracts | Verified | Refused | Claims driven | Claims passed | Conversions | Exports certified | Exports unknown |"
  );
  lines.push("| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |");
  for (const [, bucket] of Object.entries(report.byFamily)) {
    if (!bucket.rows) continue;
    lines.push(
      `| ${bucket.label} | ${bucket.rows} | ${bucket.contractsGenerated} | ${rate(bucket.verified, bucket.rows)} | ` +
        `${bucket.refused} | ${rate(bucket.claims.driven, bucket.claims.total)} | ` +
        `${rate(bucket.claims.passed, bucket.claims.driven)} | ${bucket.conversions} | ` +
        `${bucket.exports.certifiedInVerified} | ${bucket.exports.unknownInVerified} |`
    );
  }
  lines.push("");
  lines.push("| Solid target | Rows | Contracts | Verified | Refused |");
  lines.push("| --- | --- | --- | --- | --- |");
  for (const [name, bucket] of Object.entries(report.bySolidTarget)) {
    lines.push(
      `| ${name} | ${bucket.rows} | ${bucket.contractsGenerated} | ${rate(bucket.verified, bucket.rows)} | ${bucket.refused} |`
    );
  }
  lines.push("");

  lines.push("## Why verification refuses");
  lines.push("");
  lines.push(
    `${overall.refused} rows were refused. \`contract verify\` raises every blocker it finds rather than ` +
      "stopping at the first, so the row counts below sum to more than the number of refused rows."
  );
  lines.push("");
  lines.push("| Blocker (RFC 0002 §3) | Rows raising it | Blocker lines |");
  lines.push("| --- | --- | --- |");
  for (const [name, count] of sortedEntries(overall.blockerRows)) {
    lines.push(`| \`${name}\` | ${count} | ${overall.blockerLines[name] ?? 0} |`);
  }
  lines.push("");
  lines.push(
    "Attributed to one root cause per row instead. `probe-report-includes-evidence-write` is a " +
      "*consequence*: `contract probe --write` declines to write evidence once a probe failed or an " +
      "incompleteness was reported, so verification then sees passing claims that never reached the " +
      "contract. It is counted as a root cause only on a row where it stands alone."
  );
  lines.push("");
  lines.push("| Root cause | Refused rows |");
  lines.push("| --- | --- |");
  for (const [name, count] of sortedEntries(overall.rootCauses)) lines.push(`| \`${name}\` | ${count} |`);
  lines.push("");

  lines.push("## Drivability");
  lines.push("");
  lines.push("| Figure | Count |");
  lines.push("| --- | --- |");
  lines.push(`| Claims planned across every probed contract | ${overall.claims.total} |`);
  lines.push(`| Driven | ${rate(overall.claims.driven, overall.claims.total)} |`);
  lines.push(`| Passed | ${rate(overall.claims.passed, overall.claims.total)} |`);
  lines.push(`| Failed | ${overall.claims.failed} |`);
  lines.push(`| Undriven | ${rate(overall.claims.undriven, overall.claims.total)} |`);
  lines.push(`| Incompleteness findings | ${overall.claims.incompleteness} |`);
  lines.push("");
  lines.push("Undriven claims by reason:");
  lines.push("");
  lines.push("| Reason | Claims |");
  lines.push("| --- | --- |");
  for (const [name, count] of sortedEntries(overall.undriven)) lines.push(`| ${name} | ${count} |`);
  lines.push("");

  lines.push("## The probe environment");
  lines.push("");
  lines.push(
    `An entrypoint whose module cannot be imported yields no observation at all. ` +
      `${report.probeEnvironment.rowsWithAnEntrypointImportThrow} of the corpus's rows had at least one ` +
      "entrypoint import throw. The probe worker is a bare Node process: no DOM, no bundler, no JSX or " +
      "TypeScript loader, and only the packages the corpus manifest installs beside the probed one. " +
      "Some of these throws are facts about the package; others are facts about that environment, and " +
      "the two are not separated here."
  );
  lines.push("");
  lines.push("| Import failure | Claims left undriven |");
  lines.push("| --- | --- |");
  for (const [name, count] of sortedEntries(report.probeEnvironment.importThrows).slice(0, 20)) {
    lines.push(`| ${name.replace(/\|/g, "\\|")} | ${count} |`);
  }
  lines.push("");

  lines.push("## Conversion volume");
  lines.push("");
  lines.push(
    'A conversion replaces one export\'s whole claim domain with the `{"status":"unknown"}` sentinel ' +
      "because the probe neither observed nor statically proved it."
  );
  lines.push("");
  lines.push("| Figure | Count |");
  lines.push("| --- | --- |");
  lines.push(`| Claim domains converted to unknown | ${overall.conversions} |`);
  lines.push(
    `| Exports carrying an unknown in the verified rows, at generation | ${rate(overall.exports.draftUnknownInVerified, overall.exports.draftExportsInVerified)} |`
  );
  lines.push(
    `| Exports carrying an unknown in the verified rows, after verification | ${rate(overall.exports.unknownInVerified, overall.exports.certifiedInVerified + overall.exports.unknownInVerified)} |`
  );
  lines.push("");
  lines.push("How much a verified contract actually certifies from observation:");
  lines.push("");
  lines.push("| Figure | Count |");
  lines.push("| --- | --- |");
  lines.push(
    `| Verified rows carrying at least one probed behavioral row | ${rate(overall.rowsWithProbedEvidence, overall.verified)} |`
  );
  lines.push(`| Probed behavioral row markers kept across the whole corpus | ${overall.probedRowsKept} |`);
  lines.push(`| Inferred row markers dropped by verification | ${overall.droppedInferredMarkers} |`);
  lines.push(
    `| Probed markers discarded as unwitnessed by this run's report | ${overall.staleProbedMarkers} |`
  );
  lines.push("");
  lines.push("Converted domains by field:");
  lines.push("");
  lines.push("| Field | Conversions |");
  lines.push("| --- | --- |");
  for (const [name, count] of sortedEntries(overall.conversionFields)) {
    lines.push(`| \`${name}\` | ${count} |`);
  }
  lines.push("");

  const totalExports =
    overall.exports.certifiedInVerified +
    overall.exports.unknownInVerified +
    overall.exports.inUnverifiedContract;
  lines.push("## The composite a consumer feels");
  lines.push("");
  lines.push("Of every export the corpus's generated contracts describe:");
  lines.push("");
  lines.push("| State | Exports |");
  lines.push("| --- | --- |");
  lines.push(
    `| (a) certified by a verified contract | ${rate(overall.exports.certifiedInVerified, totalExports)} |`
  );
  lines.push(
    `| (b) honest unknown inside a verified contract | ${rate(overall.exports.unknownInVerified, totalExports)} |`
  );
  lines.push(
    `| (c) inside a contract that never reached \`verified\` | ${rate(overall.exports.inUnverifiedContract, totalExports)} |`
  );
  lines.push("");
  lines.push(
    "(c) is every export of a contract that was generated and then refused, timed out, or errored " +
      "before a probe report existed. Rows whose `npm install` or `contract generate` failed describe " +
      "no exports at all and are in none of the three states."
  );
  lines.push("");

  lines.push("## Wall time");
  lines.push("");
  lines.push("| Phase | Rows | Median | p90 | Max | Mean |");
  lines.push("| --- | --- | --- | --- | --- | --- |");
  for (const [name, value] of Object.entries(report.phaseWallMs)) {
    lines.push(
      `| ${name} | ${value.count} | ${value.medianMs} ms | ${value.p90Ms} ms | ${value.maxMs} ms | ${value.meanMs} ms |`
    );
  }
  lines.push("");
  lines.push(
    "`install` may run against a warm npm cache, so it is a lower bound; `pipelineWithoutInstall` is " +
      "the number that describes the checker's own cost."
  );
  lines.push("");

  const { installFailures, generateFailures, probeErrors, timeouts } = report.preContractFailures;
  lines.push("## Rows that never reached verification");
  lines.push("");
  lines.push("| Stage | Rows |");
  lines.push("| --- | --- |");
  lines.push(`| \`npm install\` failed | ${installFailures.length} |`);
  lines.push(`| \`contract generate\` failed | ${generateFailures.length} |`);
  lines.push(
    `| \`contract probe\` errored before writing a report | ${Object.values(probeErrors).reduce((sum, value) => sum + value, 0)} |`
  );
  lines.push(`| timed out under the harness budget | ${timeouts.length} |`);
  lines.push("");
  if (Object.keys(probeErrors).length) {
    lines.push("Probe errors by cause:");
    lines.push("");
    lines.push("| Cause | Rows |");
    lines.push("| --- | --- |");
    for (const [name, count] of sortedEntries(probeErrors)) lines.push(`| ${name} | ${count} |`);
    lines.push("");
  }
  if (generateFailures.length) {
    const classes = {};
    for (const failure of generateFailures) classes[failure.class] = (classes[failure.class] ?? 0) + 1;
    lines.push("Generation failures by class:");
    lines.push("");
    lines.push("| Class | Rows |");
    lines.push("| --- | --- |");
    for (const [name, count] of sortedEntries(classes)) lines.push(`| \`${name}\` | ${count} |`);
    lines.push("");
  }
  if (timeouts.length) {
    lines.push("Timeouts, named individually because a timeout is never a verification result:");
    lines.push("");
    for (const timeout of timeouts) {
      lines.push(`- \`${timeout.probeId}\` — ${timeout.outcome} after ${timeout.totalMs} ms`);
    }
    lines.push("");
  }

  lines.push("## Caveats, stated because these numbers are easy to over-read");
  lines.push("");
  lines.push(
    "- **`verified` is not `reviewed`.** A verified contract certifies what a machine observed or " +
      "statically proved and converts everything else to the unknown sentinel. It is a weaker claim " +
      "than the human `reviewed` tier, and a stronger one than the `inferred` draft the generation " +
      "benchmark measures."
  );
  lines.push(
    "- **The install environment is the corpus manifest's, and it was built for static generation.** " +
      "It installs the probed package and the Solid runtime versions the manifest selected — not the " +
      "package's full peer set. Several `ERR_MODULE_NOT_FOUND` import failures above are that gap, " +
      "not the package's."
  );
  lines.push(
    "- **A timeout is never a verification result.** Rows that exceeded the probe wall budget are " +
      "their own outcome class and are counted as neither verified nor refused."
  );
  lines.push(
    "- **Per probe row, not per package.** A package with a Solid 1.x row and two Solid 2.x rows " +
      "contributes three rows to every figure here."
  );
  lines.push(
    "- **This measurement executed package code.** Nothing here is a safety claim about any package; " +
      "it is a record of what happened when each one was imported and driven in a sandboxed child " +
      "process."
  );
  lines.push("");

  lines.push("## Every refusal");
  lines.push("");
  lines.push("| Probe | Family | Root cause | Blocker lines | Classes |");
  lines.push("| --- | --- | --- | --- | --- |");
  for (const refusal of report.refusals) {
    lines.push(
      `| \`${refusal.probeId}\` | ${refusal.family} | \`${refusal.rootCause}\` | ${refusal.blockerCount} | ${refusal.blockerClasses.join(", ")} |`
    );
  }
  lines.push("");

  lines.push("## Every verified contract");
  lines.push("");
  lines.push("| Probe | Exports | Exports unknown | Conversions | Probed rows kept |");
  lines.push("| --- | --- | --- | --- | --- |");
  for (const row of report.verified) {
    lines.push(
      `| \`${row.probeId}\` | ${row.exports} | ${row.exportsUnknown} | ${row.conversions} | ${row.probedRowsKept} |`
    );
  }
  lines.push("");

  // Every section pushes its own trailing separator, so the last one would
  // leave a blank line at end of file -- which `git diff --check` rejects.
  // Trim it here rather than special-casing the final section, so the written
  // artifact and a regeneration of it stay byte-identical.
  return `${lines.join("\n").replace(/\n+$/, "")}\n`;
}

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

function usage() {
  return `Usage: node scripts/ecosystem-benchmark/verify-corpus.mjs [options]

  --state-dir <DIR>      journal + scratch installs (default: a fresh temp dir;
                         pass an existing one to resume an interrupted run)
  --concurrency <N>      default ${DEFAULTS.concurrency}
  --every <N>            run every Nth manifest probe row (documented, deterministic
                         subsetting; the report still says how many rows ran)
  --offset <N>           offset for --every, default 0
  --ids <A,B>            run only these probe ids
  --limit <N>            stop after N selected rows
  --probe-timeout <MS>   per condition-mode child timeout (default ${DEFAULTS.probeModeTimeoutMs})
  --probe-budget <MS>    whole contract probe wall budget (default ${DEFAULTS.probeWallBudgetMs})
  --json <FILE>          default benchmarks/ecosystem/verification-report.json
  --markdown <FILE>      default benchmarks/ecosystem/verification-report.md
  --aggregate-only       rebuild the reports from the state dir's journal
  -h, --help

*** contract probe EXECUTES the installed packages' code. Run this where you
would run those packages' own test suites. ***

Requires SOLID_CHECKER_NATIVE_BIN and SOLID_TYPEFACTS_BIN to be set and to
exist. Copy both binaries somewhere stable before a long run: a concurrent
rebuild would otherwise change the engine mid-measurement, and the report
records each binary's hash as the identity the numbers belong to.
`;
}

function parseArgs(argv) {
  const options = {
    stateDir: null,
    concurrency: DEFAULTS.concurrency,
    every: null,
    offset: 0,
    ids: null,
    limit: null,
    probeModeTimeoutMs: DEFAULTS.probeModeTimeoutMs,
    probeWallBudgetMs: DEFAULTS.probeWallBudgetMs,
    json: join(REPORT_DIR, "verification-report.json"),
    markdown: join(REPORT_DIR, "verification-report.md"),
    aggregateOnly: false,
    help: false
  };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "-h" || argument === "--help") options.help = true;
    else if (argument === "--state-dir") options.stateDir = argv[++index];
    else if (argument === "--concurrency") options.concurrency = Number(argv[++index]);
    else if (argument === "--every") options.every = Number(argv[++index]);
    else if (argument === "--offset") options.offset = Number(argv[++index]);
    else if (argument === "--ids") options.ids = new Set(argv[++index].split(","));
    else if (argument === "--limit") options.limit = Number(argv[++index]);
    else if (argument === "--probe-timeout") options.probeModeTimeoutMs = Number(argv[++index]);
    else if (argument === "--probe-budget") options.probeWallBudgetMs = Number(argv[++index]);
    else if (argument === "--json") options.json = argv[++index];
    else if (argument === "--markdown") options.markdown = argv[++index];
    else if (argument === "--aggregate-only") options.aggregateOnly = true;
    else throw new Error(`unrecognized option: ${argument}`);
  }
  return options;
}

function binaryIdentity(path) {
  const bytes = readFileSync(path);
  return {
    path,
    sha256: createHash("sha256").update(bytes).digest("hex"),
    size: bytes.length,
    mtime: statSync(path).mtime.toISOString()
  };
}

function readJournal(path) {
  if (!existsSync(path)) return [];
  const records = [];
  for (const line of readFileSync(path, "utf8").split("\n")) {
    if (!line.trim()) continue;
    try {
      records.push(JSON.parse(line));
    } catch {
      // A torn final line from an interrupted run is simply not resumed.
    }
  }
  return records;
}

async function main(argv = process.argv.slice(2)) {
  let options;
  try {
    options = parseArgs(argv);
  } catch (error) {
    console.error(usage());
    console.error(`solid-checker-verify-corpus: ${error.message}`);
    process.exit(2);
    return;
  }
  if (options.help) {
    console.log(usage());
    return;
  }

  const nativeBin = process.env.SOLID_CHECKER_NATIVE_BIN;
  const typeFactsBin = process.env.SOLID_TYPEFACTS_BIN;
  for (const [name, value] of [
    ["SOLID_CHECKER_NATIVE_BIN", nativeBin],
    ["SOLID_TYPEFACTS_BIN", typeFactsBin]
  ]) {
    if (!value || !existsSync(value)) {
      console.error(`solid-checker-verify-corpus: ${name} is not set to an existing file`);
      process.exit(2);
      return;
    }
  }

  const stateDir = options.stateDir
    ? resolve(options.stateDir)
    : await mkdtemp(join(tmpdir(), "solid-checker-verify-corpus-"));
  mkdirSync(stateDir, { recursive: true });
  const workDir = join(stateDir, "work");
  mkdirSync(workDir, { recursive: true });
  const journalPath = join(stateDir, "journal.jsonl");

  const manifest = JSON.parse(readFileSync(MANIFEST, "utf8"));
  // Explicit rather than spread over `options`: the report's budget block is
  // read as the run's configuration, and a stray `--ids` Set or report path in
  // it would be noise a reader has to discount.
  const budgets = {
    installTimeoutMs: DEFAULTS.installTimeoutMs,
    generateTimeoutMs: DEFAULTS.generateTimeoutMs,
    probeModeTimeoutMs: options.probeModeTimeoutMs,
    probeWallBudgetMs: options.probeWallBudgetMs,
    verifyTimeoutMs: DEFAULTS.verifyTimeoutMs,
    concurrency: options.concurrency,
    ...(options.every ? { selection: `every ${options.every}th manifest probe row, offset ${options.offset}` } : {})
  };
  const checker = {
    note:
      "Copy both binaries somewhere stable before a long run and point the environment at the " +
      "copies, so a concurrent rebuild cannot change the engine mid-measurement.",
    nativeBin: binaryIdentity(nativeBin),
    typeFactsBin: binaryIdentity(typeFactsBin)
  };

  if (!options.aggregateOnly) {
    const { expandContract } = await import(
      pathToFileURL(join(ROOT, "packages/cli/scripts/contract-document.mjs")).href
    );
    let tasks = [];
    for (const row of sortRows([...manifest.rows])) {
      for (const probe of row.probes ?? []) tasks.push({ row, probe });
    }
    if (options.every)
      tasks = tasks.filter((_, index) => (index - options.offset) % options.every === 0);
    if (options.ids) tasks = tasks.filter(task => options.ids.has(task.probe.id));
    if (options.limit) tasks = tasks.slice(0, options.limit);

    const done = new Set(readJournal(journalPath).map(record => record.probeId));
    const pending = tasks.filter(task => !done.has(task.probe.id));
    process.stderr.write(
      `solid-checker-verify-corpus: ${tasks.length} rows selected, ${done.size} journaled, ${pending.length} to run\n`
    );

    const context = {
      workDir,
      budgets,
      cliEnv: { SOLID_CHECKER_NATIVE_BIN: nativeBin, SOLID_TYPEFACTS_BIN: typeFactsBin },
      expandContract
    };
    let cursor = 0;
    let completed = 0;
    await Promise.all(
      Array.from({ length: Math.max(1, options.concurrency) }, async () => {
        while (cursor < pending.length) {
          const task = pending[cursor++];
          const record = await runRow(task, context);
          record.finishedAt = new Date().toISOString();
          appendFileSync(journalPath, `${JSON.stringify(record)}\n`);
          completed += 1;
          process.stderr.write(
            `[${completed}/${pending.length}] ${record.probeId} -> ${record.outcome} (${record.totalMs}ms)\n`
          );
        }
      })
    );
  }

  const records = readJournal(journalPath);
  if (!records.length) {
    console.error(`solid-checker-verify-corpus: no journal records at ${journalPath}`);
    process.exit(2);
    return;
  }
  const report = buildVerificationReport({ records, manifest, budgets, checker });
  mkdirSync(dirname(options.json), { recursive: true });
  mkdirSync(dirname(options.markdown), { recursive: true });
  writeFileSync(options.json, `${JSON.stringify(report, null, 2)}\n`);
  writeFileSync(options.markdown, renderVerificationMarkdown(report));
  console.log(
    `solid-checker-verify-corpus: ${records.length} rows, ${report.overall.verified} verified, ` +
      `${report.overall.refused} refused; reports written to ${options.json} and ${options.markdown}`
  );
}

const isMain = process.argv[1] && pathToFileURL(resolve(process.argv[1])).href === import.meta.url;
if (isMain) {
  main().catch(error => {
    console.error(`solid-checker-verify-corpus: unhandled error: ${error?.stack ?? error}`);
    process.exit(2);
  });
}
