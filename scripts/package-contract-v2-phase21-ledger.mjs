#!/usr/bin/env bun

import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { buildPhase20Ledger } from "./package-contract-v2-phase20-ledger.mjs";

export const PHASE20_REPORT_SHA256 = "dfd72fa5d7e8108abdf840d0edfbb9d89cfd83df06ad03cbde2163c9e23894f2";
export const PHASE20_LEDGER_SHA256 = "1cf22736be8a71205f59cd5cd1ec02f6be0dd1c977552a89cc645b0ce8b72107";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const REPORT_PATH = join(ROOT, "benchmarks/ecosystem/report.json");
const PHASE20_LEDGER_PATH = join(ROOT, "benchmarks/package-contract-v2/phase20/row-ledger.json");
const PHASE21_DIR = join(ROOT, "benchmarks/package-contract-v2/phase21");
const BASELINE_PATH = join(PHASE21_DIR, "baseline-cohort.json");
const LEDGER_PATH = join(PHASE21_DIR, "row-ledger.json");
const MARKDOWN_PATH = join(PHASE21_DIR, "row-ledger.md");

const SEMANTIC_TERMINAL_CLASSES = new Set([
  "dependency-contract-obligation",
  "export-kind-conflict",
  "export-kind-unresolved"
]);

const CONTEXT_PROBE_ID = "@solid-primitives/context@0.3.2|solid1|only";
const GEOLOCATION_PROBE_ID = "@solid-primitives/geolocation@1.5.5|solid1|only";
const CORVU_PROBE_ID = "corvu@0.7.2|solid1|only";
const CONTEXT_DEFECT_EVIDENCE =
  "docs/package-contract-v2/phase21/context-upstream-declaration-defect.md";

const AUTHENTICATED_LAYOUT_REFUSALS = new Set([
  "@solidjs/testing-library@0.8.10|solid1|only",
  "@tanstack/solid-query@6.0.0-rc.0|solid2|floor",
  "@tanstack/solid-query@6.0.0-rc.0|solid2|head",
  "@tanstack/solid-query-persist-client@6.0.0-rc.0|solid2|floor",
  "@tanstack/solid-query-persist-client@6.0.0-rc.0|solid2|head"
]);
const PACKAGE_IMPORT_RESOLUTION_REFUSALS = new Set([
  "@tanstack/solid-start-server@1.167.36|solid1|only",
  "@tanstack/solid-start-server@2.0.0-rc.2|solid2|floor",
  "@tanstack/solid-start-server@2.0.0-rc.2|solid2|head"
]);
const SEMANTIC_MODEL_REFUSALS = new Set([
  "@tanstack/solid-db@0.2.40|solid1|only",
  "@tanstack/solid-form@2.0.0-alpha.2|solid1|only",
  "@tanstack/solid-hotkeys@0.10.0|solid1|only",
  "@tanstack/solid-query@5.102.5|solid1|only",
  "@tanstack/solid-query-persist-client@5.102.5|solid1|only"
]);
const TYPE_FACTS_CAPABILITY_REFUSALS = new Set([
  "@tanstack/solid-store@0.11.1|solid1|only",
  "@tanstack/solid-virtual@3.13.37|solid1|only"
]);

export function classifyPhase21Terminal({ observedClass, artifactCases = [] }) {
  if (SEMANTIC_TERMINAL_CLASSES.has(observedClass)) return observedClass;

  const reasons = artifactCases.map(artifactCase => artifactCase.reason ?? "").join("\n");
  if (/declarations import absent|local closure module \.\/types\.js .* was not found/i.test(reasons)) {
    return "published-declaration-closure-missing";
  }
  if (/node_modules\/solid-js\/types\/reactive\/signal\.js .* was not found/i.test(reasons)) {
    return "authenticated-dependency-layout-required";
  }
  if (/resolved target .* is not a file|package export target does not exist|archive has no dist payload/i.test(reasons)) {
    return "published-target-missing";
  }
  return observedClass;
}

function countBy(items, select) {
  const counts = {};
  for (const item of items) {
    const key = select(item);
    counts[key] = (counts[key] ?? 0) + 1;
  }
  return Object.fromEntries(Object.entries(counts).sort(([left], [right]) => left.localeCompare(right)));
}

function reportResultsByProbeId(report) {
  return new Map(
    report.results
      .filter(result => result.status !== "supplemental")
      .map(result => [result.probeId, result])
  );
}

function snapshotRow(row, result) {
  return {
    proposal: structuredClone(row.proposal),
    certification: structuredClone(row.certification),
    disposition: row.disposition,
    observedClass: row.measurement.observedClass,
    terminalClass: classifyPhase21Terminal({
      observedClass: row.measurement.observedClass,
      artifactCases: row.artifactCases
    }),
    installedVersions: structuredClone(result.installedVersions ?? {}),
    integrityVerified: result.integrityVerified === true,
    rootIntegrity: result.dependencyPlan?.rootIdentity?.integrity ?? null,
    artifactCases: structuredClone(row.artifactCases),
    dependencyPlan: structuredClone(row.dependencyPlan),
    certificationAttempt: structuredClone(result.certificationAttempt ?? null)
  };
}

function retainedControl(terminalClass, observedClass) {
  if (["published-target-missing", "published-declaration-closure-missing"].includes(terminalClass)) {
    return "upstream-missing-bytes";
  }
  if (["no-exported-surface", "cjs-only-entrypoint", "no-esm-runtime-target"].includes(observedClass)) {
    return "cjs-no-esm-model";
  }
  return null;
}

function exactRefusalDisposition(after, state, remainingOwner) {
  if (after.certification.state !== "exact-refusal") {
    return {
      state: "pending-phase21-checker-work",
      remainingOwner,
      evidence: "after.certification"
    };
  }
  return {
    state,
    remainingOwner,
    evidence: "after.certification"
  };
}

function phase21Disposition({ probeId, retainedControl: control, after }) {
  if (probeId === CONTEXT_PROBE_ID) {
    return {
      state: "confirmed-upstream-declaration-defect",
      remainingOwner: "upstream-package",
      evidence: CONTEXT_DEFECT_EVIDENCE
    };
  }
  if (probeId === CORVU_PROBE_ID) {
    if (
      after.certification.state === "verified" &&
      after.certificationAttempt?.ordinaryAnalysis?.receiptAuthenticated === true &&
      after.certificationAttempt?.ordinaryAnalysis?.exactCaseSelected === true
    ) {
      return {
        state: "verified-through-ordinary-receipt-load",
        remainingOwner: "none",
        evidence: "after.certificationAttempt.ordinaryAnalysis"
      };
    }
    return {
      state: "pending-phase21-checker-work",
      remainingOwner: "checker-dependency-composition",
      evidence: "after.certification"
    };
  }
  if (probeId === GEOLOCATION_PROBE_ID) {
    if (after.proposal.state === "partial" && after.terminalClass === "published-target-missing") {
      return {
        state: "partial-upstream-target-missing",
        remainingOwner: "upstream-package",
        evidence: "after.artifactCases"
      };
    }
    return {
      state: "pending-phase21-checker-work",
      remainingOwner: "checker-semantic-model",
      evidence: "after.proposal"
    };
  }
  if (control === "upstream-missing-bytes") {
    return {
      state: "retained-upstream-missing-bytes",
      remainingOwner: "upstream-package",
      evidence: "after.artifactCases"
    };
  }
  if (control === "cjs-no-esm-model") {
    return {
      state: "retained-unsupported-runtime-model",
      remainingOwner: "runtime-model",
      evidence: "after.artifactCases"
    };
  }
  if (AUTHENTICATED_LAYOUT_REFUSALS.has(probeId)) {
    return exactRefusalDisposition(
      after,
      "exact-refusal-authenticated-layout",
      "authenticated-dependency-layout"
    );
  }
  if (PACKAGE_IMPORT_RESOLUTION_REFUSALS.has(probeId)) {
    return exactRefusalDisposition(
      after,
      "exact-refusal-package-import-resolution",
      "checker-resolver"
    );
  }
  if (SEMANTIC_MODEL_REFUSALS.has(probeId)) {
    return exactRefusalDisposition(after, "exact-refusal-semantic-model", "checker-semantic-model");
  }
  if (TYPE_FACTS_CAPABILITY_REFUSALS.has(probeId)) {
    return exactRefusalDisposition(
      after,
      "exact-refusal-type-facts-capability",
      "checker-type-facts"
    );
  }
  throw new Error(`Phase 21 row ${probeId} has no explicit disposition`);
}

export function buildPhase21Baseline({
  phase20,
  phase20Report,
  phase20ReportSha256,
  phase20LedgerSha256
}) {
  assert.equal(phase20.documentKind, "solid-checker-package-contract-phase20-row-ledger");
  assert.equal(phase20ReportSha256, PHASE20_REPORT_SHA256, "Phase 21 must retain the exact Phase 20 report");
  assert.equal(phase20LedgerSha256, PHASE20_LEDGER_SHA256, "Phase 21 must retain the exact Phase 20 ledger");

  const baselineRows = phase20.rows.filter(row => row.proposal.state === "fully-refused");
  const baselineResults = reportResultsByProbeId(phase20Report);
  const rows = baselineRows.map(baselineRow => {
    const baselineResult = baselineResults.get(baselineRow.probeId);
    assert.ok(baselineResult, `${baselineRow.probeId} exists in the Phase 20 report`);
    return {
      probeId: baselineRow.probeId,
      package: baselineRow.package,
      version: baselineRow.version,
      family: baselineRow.family,
      solidTarget: baselineRow.solidTarget,
      probeKind: baselineRow.probeKind,
      before: snapshotRow(baselineRow, baselineResult)
    };
  });
  const baseline = {
    schemaVersion: 1,
    documentKind: "solid-checker-package-contract-phase21-baseline-cohort",
    generatedAt: phase20Report.finishedAt,
    authority: {
      phase20Report: {
        path: "benchmarks/ecosystem/report.json",
        sha256: phase20ReportSha256
      },
      phase20Ledger: {
        path: "benchmarks/package-contract-v2/phase20/row-ledger.json",
        sha256: phase20LedgerSha256
      }
    },
    rows
  };
  assertPhase21Baseline(baseline);
  return baseline;
}

export function assertPhase21Baseline(baseline) {
  assert.equal(baseline.schemaVersion, 1);
  assert.equal(baseline.documentKind, "solid-checker-package-contract-phase21-baseline-cohort");
  assert.equal(baseline.authority.phase20Report.sha256, PHASE20_REPORT_SHA256);
  assert.equal(baseline.authority.phase20Ledger.sha256, PHASE20_LEDGER_SHA256);
  assert.equal(baseline.rows.length, 30);
  assert.equal(new Set(baseline.rows.map(row => row.probeId)).size, baseline.rows.length);
  for (const row of baseline.rows) {
    assert.equal(row.before.proposal.state, "fully-refused");
    assert.ok(row.before.artifactCases.length > 0, `${row.probeId} retains its artifact cases`);
    assert.equal(row.before.installedVersions[row.package], row.version, `${row.probeId} retains installed identity`);
    assert.equal(row.before.integrityVerified, true, `${row.probeId} retains registry integrity verification`);
    assert.notEqual(row.before.terminalClass, "unclassified", `${row.probeId} has an exact terminal class`);
  }
}

export function buildPhase21LedgerFromBaseline({
  baseline,
  currentReport = null,
  currentReportSha256 = null,
  baselineSha256 = null
}) {
  assertPhase21Baseline(baseline);
  const report = currentReport ?? {
    finishedAt: baseline.generatedAt,
    results: []
  };
  const currentRows = currentReport
    ? new Map(buildPhase20Ledger(currentReport).rows.map(row => [row.probeId, row]))
    : null;
  const currentResults = currentReport ? reportResultsByProbeId(currentReport) : null;
  if (currentReport) {
    assert.match(currentReportSha256 ?? "", /^[0-9a-f]{64}$/, "current report SHA-256 is required");
  }
  const rows = baseline.rows.map(baselineRow => {
    const currentRow = currentRows?.get(baselineRow.probeId);
    const currentResult = currentResults?.get(baselineRow.probeId);
    if (currentReport) assert.ok(currentRow && currentResult, `${baselineRow.probeId} exists in the current report`);
    const before = structuredClone(baselineRow.before);
    const after = currentReport ? snapshotRow(currentRow, currentResult) : structuredClone(before);
    const control = retainedControl(before.terminalClass, before.observedClass);
    return {
      probeId: baselineRow.probeId,
      package: baselineRow.package,
      version: baselineRow.version,
      family: baselineRow.family,
      solidTarget: baselineRow.solidTarget,
      probeKind: baselineRow.probeKind,
      retainedControl: control,
      phase21Disposition: phase21Disposition({
        probeId: baselineRow.probeId,
        retainedControl: control,
        after
      }),
      before,
      after
    };
  });

  const upstreamMissingBytes = rows.filter(row => row.retainedControl === "upstream-missing-bytes").length;
  const cjsNoEsm = rows.filter(row => row.retainedControl === "cjs-no-esm-model").length;
  const ledger = {
    schemaVersion: 1,
    documentKind: "solid-checker-package-contract-phase21-row-ledger",
    generatedAt: report.finishedAt,
    authority: {
      baselineCohort: {
        path: "benchmarks/package-contract-v2/phase21/baseline-cohort.json",
        sha256: baselineSha256
      },
      currentReport: currentReport
        ? {
            path: "benchmarks/ecosystem/report.json",
            sha256: currentReportSha256
          }
        : null,
      phase20Report: structuredClone(baseline.authority.phase20Report),
      phase20Ledger: structuredClone(baseline.authority.phase20Ledger),
      rule: "a row is verified only after every applicable artifact case finalizes and an ordinary process authenticates, selects, and queries its policy-2 receipt"
    },
    summary: {
      rows: rows.length,
      terminalClasses: countBy(rows, row => row.before.terminalClass),
      currentTerminalClasses: countBy(rows, row => row.after.terminalClass),
      retainedControls: { upstreamMissingBytes, cjsNoEsm },
      checkerAddressableRows: rows.length - upstreamMissingBytes - cjsNoEsm,
      newlyVerifiedRows: rows.filter(
        row => row.before.certification.state !== "verified" && row.after.certification.state === "verified"
      ).length,
      confirmedUpstreamDefects: rows.filter(
        row => row.phase21Disposition?.state === "confirmed-upstream-declaration-defect"
      ).length,
      dispositionStates: countBy(rows, row => row.phase21Disposition.state),
      remainingOwners: countBy(rows, row => row.phase21Disposition.remainingOwner)
    },
    rows
  };
  assertPhase21Ledger(ledger);
  return ledger;
}

export function buildPhase21Ledger({
  phase20,
  phase20Report,
  currentReport = phase20Report,
  phase20ReportSha256,
  currentReportSha256 = phase20ReportSha256,
  phase20LedgerSha256
}) {
  const baseline = buildPhase21Baseline({
    phase20,
    phase20Report,
    phase20ReportSha256,
    phase20LedgerSha256
  });
  return buildPhase21LedgerFromBaseline({ baseline, currentReport, currentReportSha256 });
}

export function assertPhase21Ledger(ledger) {
  assert.equal(ledger.schemaVersion, 1);
  assert.equal(ledger.documentKind, "solid-checker-package-contract-phase21-row-ledger");
  assert.equal(ledger.rows.length, 30);
  assert.equal(new Set(ledger.rows.map(row => row.probeId)).size, ledger.rows.length);
  assert.equal(ledger.summary.rows, ledger.rows.length);
  assert.equal(ledger.summary.retainedControls.upstreamMissingBytes, 5);
  assert.equal(ledger.summary.retainedControls.cjsNoEsm, 7);
  assert.equal(ledger.summary.checkerAddressableRows, 18);
  assert.equal(ledger.summary.terminalClasses.unclassified, undefined);
  assert.equal(ledger.rows.filter(row => row.phase21Disposition == null).length, 0);
  if (ledger.authority.currentReport != null) {
    assert.match(ledger.authority.currentReport.sha256, /^[0-9a-f]{64}$/);
  }
  for (const row of ledger.rows) {
    assert.equal(row.before.proposal.state, "fully-refused");
    assert.ok(row.before.artifactCases.length > 0, `${row.probeId} retains its artifact cases`);
    assert.equal(row.before.installedVersions[row.package], row.version, `${row.probeId} retains installed identity`);
    assert.equal(row.before.integrityVerified, true, `${row.probeId} retains registry integrity verification`);
    assert.notEqual(row.before.terminalClass, "unclassified", `${row.probeId} has an exact terminal class`);
  }
  const context = ledger.rows.find(row => row.probeId === CONTEXT_PROBE_ID);
  assert.deepEqual(context.phase21Disposition, {
    state: "confirmed-upstream-declaration-defect",
    remainingOwner: "upstream-package",
    evidence: CONTEXT_DEFECT_EVIDENCE
  });
}

export function renderPhase21LedgerMarkdown(ledger) {
  const classes = Object.entries(ledger.summary.currentTerminalClasses)
    .sort((left, right) => right[1] - left[1] || left[0].localeCompare(right[0]));
  const dispositions = Object.entries(ledger.summary.dispositionStates)
    .sort((left, right) => right[1] - left[1] || left[0].localeCompare(right[0]));
  const owners = Object.entries(ledger.summary.remainingOwners)
    .sort((left, right) => right[1] - left[1] || left[0].localeCompare(right[0]));
  const cell = value => String(value).replaceAll("|", "\\|");
  return `# Phase 21 ecosystem refusal-reduction ledger

- Baseline fully refused rows: ${ledger.summary.rows}
- Current report SHA-256: ${ledger.authority.currentReport?.sha256 ?? "not bound"}
- Upstream missing-byte controls: ${ledger.summary.retainedControls.upstreamMissingBytes}
- CJS/no-ESM controls: ${ledger.summary.retainedControls.cjsNoEsm}
- Checker-addressable rows: ${ledger.summary.checkerAddressableRows}
- Newly verified rows: ${ledger.summary.newlyVerifiedRows}
- Confirmed upstream declaration defects: ${ledger.summary.confirmedUpstreamDefects}

## Current terminal classes

| Class | Rows |
| --- | ---: |
${classes.map(([name, count]) => `| ${name} | ${count} |`).join("\n")}

## Explicit dispositions

| State | Rows |
| --- | ---: |
${dispositions.map(([name, count]) => `| ${name} | ${count} |`).join("\n")}

## Remaining owners

| Owner | Rows |
| --- | ---: |
${owners.map(([name, count]) => `| ${name} | ${count} |`).join("\n")}

## Row disposition

| Probe | State | Remaining owner | Terminal class |
| --- | --- | --- | --- |
${ledger.rows
  .map(
    row =>
      `| ${cell(row.probeId)} | ${cell(row.phase21Disposition.state)} | ${cell(row.phase21Disposition.remainingOwner)} | ${cell(row.after.terminalClass)} |`
  )
  .join("\n")}
`;
}

function digestBytes(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function readJson(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

function writeLedger(baseline, currentReportBytes) {
  const baselineBytes = readFileSync(BASELINE_PATH);
  const currentReport = JSON.parse(currentReportBytes.toString("utf8"));
  const ledger = buildPhase21LedgerFromBaseline({
    baseline,
    currentReport,
    currentReportSha256: digestBytes(currentReportBytes),
    baselineSha256: digestBytes(baselineBytes)
  });
  writeFileSync(LEDGER_PATH, `${JSON.stringify(ledger, null, 2)}\n`);
  writeFileSync(MARKDOWN_PATH, renderPhase21LedgerMarkdown(ledger));
  return ledger;
}

function main() {
  const [mode] = process.argv.slice(2);
  if (mode === "--write-baseline") {
    const reportBytes = readFileSync(REPORT_PATH);
    const phase20Bytes = readFileSync(PHASE20_LEDGER_PATH);
    const baseline = buildPhase21Baseline({
      phase20: JSON.parse(phase20Bytes.toString("utf8")),
      phase20Report: JSON.parse(reportBytes.toString("utf8")),
      phase20ReportSha256: digestBytes(reportBytes),
      phase20LedgerSha256: digestBytes(phase20Bytes)
    });
    mkdirSync(PHASE21_DIR, { recursive: true });
    writeFileSync(BASELINE_PATH, `${JSON.stringify(baseline, null, 2)}\n`);
    const ledger = writeLedger(baseline, reportBytes);
    console.log(`froze and wrote Phase 21 ledger for ${ledger.summary.rows} rows`);
    return;
  }
  if (mode === "--write") {
    const ledger = writeLedger(readJson(BASELINE_PATH), readFileSync(REPORT_PATH));
    console.log(`wrote Phase 21 ledger for ${ledger.summary.rows} rows`);
    return;
  }
  if (mode === "--check") {
    const baselineBytes = readFileSync(BASELINE_PATH);
    const reportBytes = readFileSync(REPORT_PATH);
    const baseline = JSON.parse(baselineBytes.toString("utf8"));
    const ledger = buildPhase21LedgerFromBaseline({
      baseline,
      currentReport: JSON.parse(reportBytes.toString("utf8")),
      currentReportSha256: digestBytes(reportBytes),
      baselineSha256: digestBytes(baselineBytes)
    });
    assert.deepEqual(readJson(LEDGER_PATH), ledger);
    assert.equal(readFileSync(MARKDOWN_PATH, "utf8"), renderPhase21LedgerMarkdown(ledger));
    console.log(`checked Phase 21 ledger for ${ledger.summary.rows} rows`);
    return;
  }
  throw new Error(
    "usage: bun scripts/package-contract-v2-phase21-ledger.mjs --write-baseline | --write | --check"
  );
}

if (import.meta.main) main();
