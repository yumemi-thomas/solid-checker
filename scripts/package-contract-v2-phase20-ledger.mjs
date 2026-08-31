#!/usr/bin/env bun

import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { classifyResult } from "./ecosystem-benchmark/lib/classify.mjs";
import { collectExternalEdges } from "./ecosystem-benchmark/lib/external-edges.mjs";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const REPORT_RELATIVE = "benchmarks/ecosystem/report.json";
const PHASE20 = join(ROOT, "benchmarks/package-contract-v2/phase20");
const LEDGER_PATH = join(PHASE20, "row-ledger.json");
const MARKDOWN_PATH = join(PHASE20, "row-ledger.md");
const PHASE21_BASELINE_PATH = join(
  ROOT,
  "benchmarks/package-contract-v2/phase21/baseline-cohort.json"
);

export const PROPOSAL_STATES = ["complete", "partial", "fully-refused"];
export const CERTIFICATION_STATES = ["not-attempted", "exact-refusal", "verified"];
export const APPLICABILITY_CLASSES = [
  "runtime-module",
  "verifier-proved-type-only",
  "unavailable-published-target",
  "unsupported-condition-environment",
  "unsupported-artifact-shape"
];

const BLOCKER_SLICE = new Map([
  ["exact-proposal-identity", 2],
  ["normalized-proposal-subject", 2],
  ["export-local-identity", 2],
  ["local-module-resolution", 3],
  ["finite-branch-census", 3],
  ["artifact-applicability", 4],
  ["canonical-closure-replay", 5],
  ["archive-topology", 5],
  ["certification-authority", 6],
  ["export-value-type-facts", 7],
  ["receipt-publication-load", 8],
  ["export-kind-census", 11],
  ["geolocation-reconciliation", 12],
  ["dependency-composition", 13],
  ["artifact-model", 14],
  ["upstream-artifact-or-manual-triage", 15]
]);

function countBy(items, select) {
  const counts = {};
  for (const item of items) {
    const key = select(item);
    counts[key] = (counts[key] ?? 0) + 1;
  }
  return Object.fromEntries(Object.entries(counts).sort(([left], [right]) => left.localeCompare(right)));
}

function digestBytes(value) {
  return createHash("sha256").update(value).digest("hex");
}

function effectiveClassification(result) {
  if (!["failure"].includes(result.outcome)) {
    return { class: result.class, detail: result.detail ?? {}, reclassified: false };
  }
  const classified = classifyResult({
    status: result.exitStatus,
    stdout: result.stdout ?? "",
    stderr: result.stderr ?? "",
    timedOut: result.timedOut ?? false,
    phase: "generate"
  });
  return {
    class: classified.class,
    detail: classified.detail,
    reclassified: classified.class !== result.class
  };
}

export function classifyArtifactApplicability({ accepted = false, reason = "" }) {
  if (accepted) return "runtime-module";
  if (/verifier-proved type-only|authenticated type-only leaf/i.test(reason)) {
    return "verifier-proved-type-only";
  }
  if (
    /resolved target .* is not a file|package export target does not exist|local closure module .* was not found|archive has no dist payload|declarations import absent/i.test(reason)
  ) {
    return "unavailable-published-target";
  }
  if (
    /overlapping conditional-export branches|incompatible semantics across conditional targets|environment-dependent export kind|unsupported condition|custom condition/i.test(reason)
  ) {
    return "unsupported-condition-environment";
  }
  if (
    /has only a CJS runtime target|has no (?:supported )?runtime ESM|has no runtime ESM exports|no declaration target exists|\.d\.(?:ts|mts|cts).*not part of the TypeScript project|Unknown file extension|(?:\?|:)raw\b|\bCSS\b|worker handler|side-effect-only/i.test(reason)
  ) {
    return "unsupported-artifact-shape";
  }
  return "runtime-module";
}

function proposalState(result) {
  if (result.outcome === "success") return "complete";
  if (result.outcome === "partial-success") return "partial";
  return "fully-refused";
}

function certificationState(attempt) {
  if (!attempt?.attempted) return "not-attempted";
  if (
    attempt.status === "certified" &&
    attempt.ordinaryAnalysis?.receiptAuthenticated === true &&
    attempt.ordinaryAnalysis?.exactCaseSelected === true
  ) return "verified";
  return "exact-refusal";
}

function acceptedCases(result) {
  const recorded = result.contractContent?.artifactCases;
  if (Array.isArray(recorded)) {
    return recorded.map((artifactCase, index) => ({
      id: `accepted:${index}`,
      source: "proposal-document",
      stage: "accepted-proposal-case",
      ...artifactCase,
      reason: null,
      applicability: "runtime-module"
    }));
  }
  const count = result.contractContent?.artifactCasesTotal ?? 0;
  return Array.from({ length: count }, (_, index) => ({
    id: `accepted:${index}`,
    source: "legacy-aggregate-placeholder",
    stage: "accepted-proposal-case",
    entrypoint: null,
    caseIndex: index,
    artifact: null,
    declarations: null,
    resolution: null,
    reason: null,
    applicability: "runtime-module"
  }));
}

function refusalCases(result) {
  const topLevel = Array.isArray(result.artifactCaseRefusals)
    ? result.artifactCaseRefusals
    : null;
  const nested = Array.isArray(result.contractContent?.artifactCaseRefusals)
    ? result.contractContent.artifactCaseRefusals
    : null;
  const refusals = topLevel ?? nested;
  if (refusals) {
    return {
      censusComplete: true,
      cases: refusals.map((refusal, index) => ({
        id: `refused:${index}`,
        source: topLevel ? "proposal-refusal-audit" : "legacy-proposal-refusal-audit",
        ...refusal,
        applicability: APPLICABILITY_CLASSES.includes(refusal.applicability)
          ? refusal.applicability
          : classifyArtifactApplicability({ reason: refusal.reason })
      }))
    };
  }
  if (result.outcome !== "failure") return { censusComplete: true, cases: [] };
  const reason = result.stderr || result.signature || "full refusal without retained structured audit";
  return {
    censusComplete: false,
    cases: [{
      id: "terminal:0",
      source: "legacy-terminal-only",
      entrypoint: result.detail?.entrypoint ?? null,
      conditions: null,
      stage: "row-terminal",
      reason,
      applicability: classifyArtifactApplicability({ reason })
    }]
  };
}

function membershipsFor(row, effective, artifactCases) {
  const memberships = new Set();
  const text = [
    row.stderr ?? "",
    row.certificationAttempt?.reason ?? "",
    ...artifactCases.map(artifactCase => artifactCase.reason ?? "")
  ].join("\n");
  if (/inference has no entrypoint/i.test(text)) memberships.add("exact-proposal-identity");
  if (/proposal closure|subject .* no longer exists|normalized operation graph.*subject/i.test(text)) {
    memberships.add("normalized-proposal-subject");
  }
  if (/local closure module .* was not found|extensionless|\.dev\.(?:jsx|tsx)|declaration target/i.test(text)) {
    memberships.add("local-module-resolution");
  }
  if (/wildcard|resource limit|artifact-case candidates/i.test(text)) memberships.add("finite-branch-census");
  if (/module closure mismatch/i.test(text)) memberships.add("canonical-closure-replay");
  if (/duplicate archive member|canonical alias/i.test(text)) memberships.add("archive-topology");
  if (/runtime export .* declaration|exact declaration binding|PackageContractExportMissing/i.test(text)) {
    memberships.add("export-local-identity");
  }
  if (artifactCases.some(artifactCase => artifactCase.applicability !== "runtime-module")) {
    memberships.add("artifact-applicability");
  }
  if (artifactCases.some(artifactCase => artifactCase.applicability === "unavailable-published-target")) {
    memberships.add("upstream-artifact-or-manual-triage");
  }
  if (/CJS|no (?:supported )?runtime ESM|no runtime ESM exports|Unknown file extension|(?:\?|:)raw\b|worker|side-effect/i.test(text)) {
    memberships.add("artifact-model");
  }
  if (effective.class === "dependency-contract-obligation") memberships.add("dependency-composition");
  if (["export-kind-unresolved", "export-kind-conflict"].includes(effective.class)) {
    memberships.add("export-kind-census");
  }
  if (row.package === "@solid-primitives/geolocation" && effective.class === "export-kind-conflict") {
    memberships.add("geolocation-reconciliation");
  }
  if (effective.class === "package-contract-export-missing") memberships.add("export-local-identity");
  if (["no-exported-surface", "cjs-only-entrypoint", "no-esm-runtime-target"].includes(effective.class)) {
    memberships.add("artifact-model");
  }
  if (row.certificationAttempt?.attempted) memberships.add("certification-authority");
  if (memberships.size === 0) memberships.add("upstream-artifact-or-manual-triage");
  return [...memberships].sort((left, right) => {
    const slice = (BLOCKER_SLICE.get(left) ?? 99) - (BLOCKER_SLICE.get(right) ?? 99);
    return slice || left.localeCompare(right);
  });
}

function nextOwner(row, memberships) {
  if (row.package === "@solid-primitives/geolocation" && memberships.includes("geolocation-reconciliation")) {
    return { slice: 12, blocker: "geolocation-reconciliation" };
  }
  const blocker = memberships[0];
  return { slice: BLOCKER_SLICE.get(blocker) ?? 15, blocker };
}

function exactExternalEdges(result, artifactCases) {
  if (Array.isArray(result.externalEdges)) return result.externalEdges;
  const derived = collectExternalEdges({
    texts: [result.stderr ?? "", ...artifactCases.map(artifactCase => artifactCase.reason ?? "")]
  });
  return derived.map(edge => ({
    ...edge,
    resolvedVersion: result.installedVersions?.[edge.package] ?? edge.resolvedVersion
  }));
}

function proofFamilies(attempt) {
  const names = new Set([
    ...Object.keys(attempt?.demandCountsByFamily ?? {}),
    ...Object.keys(attempt?.artifactSatisfiedDemandsByFamily ?? {}),
    ...Object.keys(attempt?.refusalCountsByFamily ?? {})
  ]);
  return [...names].sort().map(family => ({
    family,
    demanded: attempt?.demandCountsByFamily?.[family] ?? 0,
    artifactSatisfied: attempt?.artifactSatisfiedDemandsByFamily?.[family] ?? 0,
    refused: attempt?.refusalCountsByFamily?.[family] ?? 0
  }));
}

function buildRow(result) {
  const effective = effectiveClassification(result);
  const accepted = acceptedCases(result);
  const refused = refusalCases(result);
  const artifactCases = [...accepted, ...refused.cases];
  const memberships = membershipsFor(result, effective, artifactCases);
  const applicabilityCounts = countBy(artifactCases, artifactCase => artifactCase.applicability);
  const applicabilityKinds = Object.keys(applicabilityCounts);
  const certification = certificationState(result.certificationAttempt);
  return {
    probeId: result.probeId,
    package: result.package,
    version: result.version,
    family: result.family,
    solidTarget: result.solidTarget,
    probeKind: result.probeKind,
    proposal: {
      state: proposalState(result),
      acceptedArtifactCases: accepted.length,
      refusedArtifactCases: refused.cases.length,
      refusalCensusComplete: refused.censusComplete,
      acceptedCaseIdentityComplete: accepted.every(artifactCase => artifactCase.source === "proposal-document")
    },
    certification: {
      state: certification,
      attempted: result.certificationAttempt?.attempted === true,
      status: result.certificationAttempt?.status ?? null,
      stage: result.certificationAttempt?.stage ?? null,
      owner: result.certificationAttempt?.owner ?? null,
      reason: result.certificationAttempt?.reason ?? null,
      proofFamilies: proofFamilies(result.certificationAttempt)
    },
    applicability: {
      aggregate: applicabilityKinds.length === 1 ? applicabilityKinds[0] : "mixed",
      counts: applicabilityCounts
    },
    artifactCases,
    externalEdges: exactExternalEdges(result, artifactCases),
    dependencyPlan: result.dependencyPlan ?? null,
    blockerMemberships: memberships,
    nextOwner: nextOwner(result, memberships),
    disposition:
      certification === "verified"
        ? "verified-through-ordinary-receipt-load"
        : proposalState(result) === "complete"
          ? certification === "exact-refusal"
            ? "proposal-complete-certification-refused"
            : "proposal-complete-certification-not-attempted"
          : "proposal-blocked",
    measurement: {
      baselineOutcome: result.outcome,
      baselineClass: result.class,
      observedClass: effective.class,
      classifierCorrected: effective.reclassified
    }
  };
}

export function buildPhase20Ledger(ecosystem, { reportPath = REPORT_RELATIVE, reportSha256 = null } = {}) {
  const official = ecosystem.results.filter(result => result.status !== "supplemental");
  const rows = official.map(buildRow);
  const proposalStates = countBy(rows, row => row.proposal.state);
  const certificationStates = countBy(rows, row => row.certification.state);
  const failureRows = rows.filter(row => row.proposal.state === "fully-refused");
  const blockerMemberships = {};
  for (const row of rows) {
    for (const blocker of row.blockerMemberships) {
      blockerMemberships[blocker] = (blockerMemberships[blocker] ?? 0) + 1;
    }
  }
  const ledger = {
    schemaVersion: 1,
    documentKind: "solid-checker-package-contract-phase20-row-ledger",
    generatedAt: ecosystem.finishedAt,
    authority: {
      report: { path: reportPath, sha256: reportSha256 },
      rule: "proposal progress, certification state, and artifact applicability are orthogonal; only an authenticated ordinary receipt load is verified"
    },
    baseline: {
      reportSha256: "45a9dd28f6360ba9438d69d6153b99a01bdd8801dd6041f4ba230bf1b4495c15",
      proposalStates: { complete: 44, partial: 314, "fully-refused": 60 },
      failureLedgers: {
        dependencyContractObligation: 21,
        exportKindUnresolved: 15,
        geolocationExportKindConflict: 1
      }
    },
    summary: {
      rows: rows.length,
      proposalStates,
      certificationStates,
      applicabilityCases: countBy(rows.flatMap(row => row.artifactCases), artifactCase => artifactCase.applicability),
      mixedApplicabilityRows: rows.filter(row => row.applicability.aggregate === "mixed").length,
      incompleteRefusalCensusRows: rows.filter(row => !row.proposal.refusalCensusComplete).length,
      incompleteAcceptedCaseIdentityRows: rows.filter(row => !row.proposal.acceptedCaseIdentityComplete).length,
      externalEdges: rows.reduce((total, row) => total + row.externalEdges.length, 0),
      externalEdgesWithoutResolvedVersion: rows.reduce(
        (total, row) => total + row.externalEdges.filter(edge => edge.resolvedVersion === null).length,
        0
      ),
      dependencyPlannedRows: rows.filter(row => row.dependencyPlan !== null).length,
      incompleteDependencyPlanRows: rows.filter(
        row => row.externalEdges.length > 0 && row.dependencyPlan?.complete !== true
      ).length,
      blockerMemberships: Object.fromEntries(Object.entries(blockerMemberships).sort(([left], [right]) => left.localeCompare(right))),
      failureLedgers: {
        dependencyContractObligation: failureRows.filter(row => row.measurement.observedClass === "dependency-contract-obligation").length,
        exportKindUnresolved: failureRows.filter(row => row.measurement.observedClass === "export-kind-unresolved").length,
        geolocationExportKindConflict: failureRows.filter(
          row => row.package === "@solid-primitives/geolocation" && row.measurement.observedClass === "export-kind-conflict"
        ).length
      },
      classifierCorrections: rows.filter(row => row.measurement.classifierCorrected).length,
      verifiedRows: rows.filter(row => row.certification.state === "verified").length
    },
    rows
  };
  assertPhase20Ledger(ledger);
  return ledger;
}

export function assertPhase20Ledger(ledger) {
  assert.equal(ledger.schemaVersion, 1);
  assert.equal(ledger.documentKind, "solid-checker-package-contract-phase20-row-ledger");
  assert.equal(ledger.rows.length, ledger.summary.rows);
  assert.equal(new Set(ledger.rows.map(row => row.probeId)).size, ledger.rows.length);
  for (const row of ledger.rows) {
    assert.ok(PROPOSAL_STATES.includes(row.proposal.state), `${row.probeId} has one proposal state`);
    assert.ok(CERTIFICATION_STATES.includes(row.certification.state), `${row.probeId} has one certification state`);
    assert.ok(row.artifactCases.length > 0, `${row.probeId} has an applicability census`);
    for (const artifactCase of row.artifactCases) {
      assert.ok(
        APPLICABILITY_CLASSES.includes(artifactCase.applicability),
        `${row.probeId}/${artifactCase.id} has one applicability class`
      );
    }
    const classes = new Set(row.artifactCases.map(artifactCase => artifactCase.applicability));
    assert.equal(row.applicability.aggregate, classes.size === 1 ? [...classes][0] : "mixed");
    assert.ok(row.blockerMemberships.length > 0);
    if (row.certification.state === "verified") {
      assert.equal(row.disposition, "verified-through-ordinary-receipt-load");
    }
    if (row.externalEdges.length > 0) {
      assert.ok(row.dependencyPlan, `${row.probeId} has a dependency plan`);
      if (row.dependencyPlan.complete !== true) {
        assert.equal(
          row.dependencyPlan.status,
          "resource-refusal",
          `${row.probeId} has either a complete plan or an explicit resource refusal`
        );
      }
      assert.ok(row.dependencyPlan.leaves.length > 0 || row.dependencyPlan.cycles.length > 0);
    }
  }
}

export function renderPhase20LedgerMarkdown(ledger) {
  const summary = ledger.summary;
  const rows = Object.entries(summary.blockerMemberships)
    .sort((left, right) => right[1] - left[1] || left[0].localeCompare(right[0]));
  return `# Phase 20 ecosystem row ledger

- Rows: ${summary.rows}
- Proposal states: ${summary.proposalStates.complete ?? 0} complete, ${summary.proposalStates.partial ?? 0} partial, ${summary.proposalStates["fully-refused"] ?? 0} fully refused
- Certification states: ${summary.certificationStates.verified ?? 0} verified, ${summary.certificationStates["exact-refusal"] ?? 0} exact refusal, ${summary.certificationStates["not-attempted"] ?? 0} not attempted
- Historical full-refusal rows awaiting structured remeasurement: ${summary.incompleteRefusalCensusRows}
- Accepted-case rows awaiting identity-rich remeasurement: ${summary.incompleteAcceptedCaseIdentityRows}
- External edges without an observed exact installed version: ${summary.externalEdgesWithoutResolvedVersion}/${summary.externalEdges}
- Complete recursive dependency plans: ${summary.dependencyPlannedRows - summary.incompleteDependencyPlanRows}/${summary.dependencyPlannedRows}
- Classifier corrections: ${summary.classifierCorrections}

## Live failure ledgers

| Ledger | Rows |
| --- | ---: |
| dependency contract obligation | ${summary.failureLedgers.dependencyContractObligation} |
| export kind unresolved | ${summary.failureLedgers.exportKindUnresolved} |
| geolocation export kind conflict | ${summary.failureLedgers.geolocationExportKindConflict} |

## Overlapping blocker memberships

These memberships deliberately overlap and therefore do not sum to ${summary.rows}.

| Blocker | Rows |
| --- | ---: |
${rows.map(([name, count]) => `| ${name} | ${count} |`).join("\n")}
`;
}

function loadReport() {
  const reportBytes = readFileSync(join(ROOT, REPORT_RELATIVE));
  return {
    report: JSON.parse(reportBytes.toString("utf8")),
    sha256: digestBytes(reportBytes)
  };
}

export function assertFrozenPhase20Ledger({ ledgerBytes, markdown, phase21Baseline }) {
  assert.equal(
    phase21Baseline.documentKind,
    "solid-checker-package-contract-phase21-baseline-cohort"
  );
  assert.equal(
    digestBytes(ledgerBytes),
    phase21Baseline.authority.phase20Ledger.sha256,
    "the frozen Phase 20 ledger must match the Phase 21 baseline authority"
  );
  const ledger = JSON.parse(ledgerBytes.toString("utf8"));
  assertPhase20Ledger(ledger);
  assert.equal(markdown, renderPhase20LedgerMarkdown(ledger));
  return ledger;
}

function main() {
  const [mode] = process.argv.slice(2);
  if (mode === "--check" && existsSync(PHASE21_BASELINE_PATH)) {
    const ledger = assertFrozenPhase20Ledger({
      ledgerBytes: readFileSync(LEDGER_PATH),
      markdown: readFileSync(MARKDOWN_PATH, "utf8"),
      phase21Baseline: JSON.parse(readFileSync(PHASE21_BASELINE_PATH, "utf8"))
    });
    console.log(`checked frozen Phase 20 ledger for ${ledger.summary.rows} rows`);
    return;
  }
  const { report, sha256 } = loadReport();
  const ledger = buildPhase20Ledger(report, { reportSha256: sha256 });
  const markdown = renderPhase20LedgerMarkdown(ledger);
  if (mode === "--write") {
    mkdirSync(PHASE20, { recursive: true });
    writeFileSync(LEDGER_PATH, `${JSON.stringify(ledger, null, 2)}\n`);
    writeFileSync(MARKDOWN_PATH, markdown);
    console.log(`wrote Phase 20 ledger for ${ledger.summary.rows} rows`);
    return;
  }
  if (mode === "--check") {
    assert.deepEqual(JSON.parse(readFileSync(LEDGER_PATH, "utf8")), ledger);
    assert.equal(readFileSync(MARKDOWN_PATH, "utf8"), markdown);
    console.log(`checked Phase 20 ledger for ${ledger.summary.rows} rows`);
    return;
  }
  throw new Error("usage: bun scripts/package-contract-v2-phase20-ledger.mjs --write | --check");
}

if (import.meta.main) main();
