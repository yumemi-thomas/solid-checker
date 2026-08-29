#!/usr/bin/env bun

import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import {
  auditPhase19Cut,
  auditPhase19DemandAuthority
} from "./package-contract-phase19.mjs";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const phase19 = join(root, "benchmarks/package-contract-v2/phase19");
const reportPath = join(phase19, "report.json");
const markdownPath = join(phase19, "report.md");

const inputs = Object.freeze({
  ecosystem: "benchmarks/ecosystem/report.json",
  phase16Refusals: "benchmarks/package-contract-v2/phase16/refusals.json",
  baseline: "docs/package-contract-v2/phase19/policy1-baseline.json",
  migration: "docs/package-contract-v2/phase19/receipt-migration.json",
  demandAuthority: "docs/package-contract-v2/phase19/proof-demand-authority-audit.json",
  policy: "docs/package-contract-v2/phase19/proof-policy-v2.json"
});

const domainOwners = Object.freeze({
  callbacks: "type-facts/call-target-and-callback-execution",
  reads: "type-facts/reactive-read-reachability",
  writes: "type-facts/reactive-write-reachability",
  creates: "type-facts/resource-creation-and-ownership",
  invalidates: "type-facts/invalidation-reachability",
  throws: "type-facts/error-edge-and-throw-census",
  returns: "type-facts/recursive-return-value",
  cleanups: "type-facts/cleanup-edge-and-lifetime",
  disposals: "type-facts/resource-disposal-and-lifetime",
  recursiveValue: "type-facts/exact-recursive-value-leaf"
});

function bytes(path) {
  return readFileSync(join(root, path));
}

function json(path) {
  return JSON.parse(bytes(path).toString("utf8"));
}

function sha256(path) {
  return createHash("sha256").update(bytes(path)).digest("hex");
}

function countsBy(items, key) {
  return Object.fromEntries(
    [...new Set(items.map(item => item[key]))]
      .sort()
      .map(value => [value, items.filter(item => item[key] === value).length])
  );
}

function unobservedDistribution(unit, reason) {
  return {
    count: 0,
    p50: null,
    p95: null,
    max: null,
    unit,
    observationStatus: reason
  };
}

function observedDistribution(values, unit, emptyReason) {
  const sorted = values.filter(Number.isFinite).slice().sort((left, right) => left - right);
  if (sorted.length === 0) return unobservedDistribution(unit, emptyReason);
  const at = fraction => sorted[Math.max(0, Math.ceil(sorted.length * fraction) - 1)];
  return {
    count: sorted.length,
    p50: at(0.5),
    p95: at(0.95),
    max: sorted.at(-1),
    unit,
    observationStatus: "observed"
  };
}

function sumNamedCounts(attempts, field, name) {
  return attempts.reduce((total, attempt) => total + (attempt[field]?.[name] ?? 0), 0);
}

function demandFamilies(audit, attempts) {
  return [...new Set(audit.demands.map(demand => demand.family))]
    .sort()
    .map(family => {
      const demands = audit.demands.filter(demand => demand.family === family);
      const statuses = countsBy(demands, "status");
      return {
        family,
        applicableAuthorityDemands: demands.length,
        alreadyExact: statuses["already exact"] ?? 0,
        producerExtensionRequired: statuses["producer extension required"] ?? 0,
        unsupported: statuses.unsupported ?? 0,
        observedCertificationDemands: sumNamedCounts(
          attempts,
          "demandCountsByFamily",
          family
        ),
        artifactSnapshotSatisfiedDemands: sumNamedCounts(
          attempts,
          "artifactSatisfiedDemandsByFamily",
          family
        ),
        refusedDemandInstances: sumNamedCounts(
          attempts,
          "refusalCountsByFamily",
          family
        ),
        authenticatedEvidenceBytes: 0,
        observationStatus:
          sumNamedCounts(attempts, "demandCountsByFamily", family) > 0
            ? "attempted-opaque-authority-no-serialized-evidence-bytes"
            : "not-applicable-to-attempted-rows"
      };
    });
}

export function buildPhase19Report(sources) {
  const { ecosystem, phase16Refusals, baseline, migration, demandAuthority } = sources;
  const official = ecosystem.results.filter(result => result.status !== "supplemental");
  const status = countsBy(official, "outcome");
  const content = ecosystem.combined.contractContent;
  const attempts = official.flatMap(result =>
    result.certificationAttempt?.attempted ? [result.certificationAttempt] : []
  );
  const attemptStatuses = countsBy(attempts, "status");
  const attemptOwners = countsBy(attempts, "owner");
  const failureClasses = countsBy(phase16Refusals.generatedProposalFailures, "class");
  const finiteWildcardBaselineRows = phase16Refusals.generatedProposalFailures.filter(
    row =>
      row.class === "unclassified" &&
      (/wildcard export/.test(row.reason) || /package exports .*\*/.test(row.reason))
  );
  const currentByProbe = new Map(official.map(result => [result.probeId, result]));
  const finiteWildcardRemeasurement = finiteWildcardBaselineRows.map(row => {
    const current = currentByProbe.get(row.probeId);
    assert.ok(current, `finite-wildcard baseline row ${row.probeId} is missing from the current corpus`);
    return {
      probeId: row.probeId,
      outcome: current.outcome,
      class: current.class,
      verifiedExportsUnlocked: 0
    };
  });
  const finiteWildcardOutcomes = countsBy(finiteWildcardRemeasurement, "outcome");
  const resourceLimitRefusals = official.flatMap(
    result => result.contractContent?.artifactCaseRefusals ?? []
  ).filter(refusal => /resource limit/.test(refusal.reason));
  const cut = auditPhase19Cut(root);
  const authority = auditPhase19DemandAuthority(root);

  assert.equal(official.length, 418);
  assert.equal(status.success + status["partial-success"] + status.failure, official.length);
  assert.ok(status.success + status["partial-success"] >= 355);
  assert.equal(status.failure, 60);
  assert.equal(attempts.length, status.success);
  assert.equal(migration.rows.length, 73);
  assert.equal(migration.pending, 0);
  assert.equal(cut.activePolicy2Receipts, 0);
  assert.equal(authority.demands, demandAuthority.demands.length);

  const openClaims = Object.entries(content.unknownByDomain)
    .filter(([, count]) => count > 0)
    .map(([domain, count]) => ({
      owner: domainOwners[domain] ?? "semantic-authority",
      recursivePath: `/exports/*/${domain}`,
      count,
      status: "open-proposal-claim"
    }));
  const totalPositiveFacts = Object.values(content.behavioralRows).reduce(
    (total, count) => total + count,
    0
  );

  return {
    schemaVersion: 1,
    documentKind: "solid-checker-package-contract-phase19-report",
    generatedAt: "2026-08-30",
    authority: {
      policyVersion: cut.proofVersion,
      receiptVersion: cut.receiptVersion,
      policyDigest: cut.policyDigest,
      proofRule:
        "no row, export, fact, closure, or receipt is counted as verified without authenticated policy-2 authority",
      inputs: Object.fromEntries(
        Object.entries(inputs).map(([name, path]) => [name, { path, sha256: sha256(path) }])
      )
    },
    artifactStatus: {
      denominator: official.length,
      denominatorMeaning: "official ecosystem rows",
      completeProposal: status.success,
      partialProposal: status["partial-success"],
      fullRefusal: status.failure,
      notApplicable: 0
    },
    policy2Issuance: {
      packages: {
        issued: 0,
        denominator: content.packagesMeasured,
        denominatorMeaning: "ecosystem packages with a generated proposal"
      },
      artifactCases: {
        issued: 0,
        denominator: baseline.inventory.receiptIssuedArtifactCases,
        denominatorMeaning: "artifact cases previously issued under policy 1"
      },
      activeReceiptDocuments: {
        issued: cut.activePolicy2Receipts,
        denominator: baseline.inventory.activeReceiptDocuments,
        denominatorMeaning: "active policy-1 receipt documents at the Phase 19 baseline"
      }
    },
    certificationAttempts: {
      structurallyCompleteRows: status.success,
      attempted: attempts.length,
      outcomes: attemptStatuses,
      firstRefusalOwners: attemptOwners,
      newlyCertified: attemptStatuses.certified ?? 0,
      reason:
        "every structurally complete row reached snapshot-bound certification planning; exact missing live authorities refused issuance"
    },
    verifiedSemantics: {
      proposalExports: content.exportsTotal,
      policy2VerifiedExports: 0,
      proposalPositiveFacts: totalPositiveFacts,
      analyzerVisiblePolicy2PositiveFacts: 0,
      locallyClosedClaimDomains: 0,
      openClaims
    },
    verifiedExportLeverage: {
      totalUnlocked: 0,
      byNewFactOrDependencyReceipt: [],
      finiteWildcardCensus: {
        historicalRows: finiteWildcardBaselineRows.length,
        otherArtifactShapeRows:
          (failureClasses.unclassified ?? 0) - finiteWildcardBaselineRows.length,
        implementationStatus: "implemented-and-remeasured",
        currentOutcomes: finiteWildcardOutcomes,
        rows: finiteWildcardRemeasurement,
        verifiedExportsUnlocked: finiteWildcardRemeasurement.reduce(
          (total, row) => total + row.verifiedExportsUnlocked,
          0
        )
      }
    },
    refusalOwnerQueue: [
      {
        order: 1,
        owner: "accepted-dependency-composition",
        rows: failureClasses["dependency-contract-obligation"] ?? 0,
        disposition: "open-no-policy2-dependency-receipts"
      },
      {
        order: 2,
        owner: "type-facts/export-kind-census",
        rows: failureClasses["export-kind-unresolved"] ?? 0,
        disposition: "open-producer-evidence-required"
      },
      {
        order: 3,
        owner: "type-facts/parameter-behavior",
        rows: failureClasses["unresolved-parameter-behavior"] ?? 0,
        disposition: "open-producer-evidence-required"
      },
      {
        order: 4,
        owner: "artifact-resolver/export-identity",
        rows: failureClasses["package-contract-export-missing"] ?? 0,
        disposition: "open-resolution-repair-required"
      },
      {
        order: 5,
        owner: "artifact-resolver/finite-wildcard-census",
        rows: failureClasses.unclassified ?? 0,
        disposition:
          "finite-wildcard-subset-remeasured-with-deeper-exact-refusals; other artifact shapes open"
      },
      {
        order: 6,
        owner: "artifact-model/no-esm-surface",
        rows: failureClasses["no-exported-surface"] ?? 0,
        disposition: "retained-refusal"
      }
    ],
    policyStrengthRefusals: {
      baselinePolicy1Receipts: migration.baselineReceipts,
      reissued: migration.reissued,
      retiredOrDemoted: migration.retired,
      pending: migration.pending,
      introducedByPolicy2: migration.retired,
      owner: "probe-gate",
      reason: "mandatory policy-2 probe harness binding unavailable"
    },
    refusalCategories: [
      {
        category: "artifact-provenance",
        count: attemptOwners["artifact-provenance"] ?? 0,
        scope: "policy2 certification attempts",
        note: "attempts that did not reach an authenticated immutable snapshot and demand graph"
      },
      {
        category: "producer-session",
        count:
          sumNamedCounts(attempts, "refusalCountsByOwner", "type-facts") +
          sumNamedCounts(attempts, "refusalCountsByOwner", "compiler-facts"),
        scope: "policy2 certification demand instances",
        authorityDefinitionsOpen: authority.producerExtensionRequired + authority.unsupported,
        note: "exact live producer demands refused across attempted rows"
      },
      {
        category: "probe-gate",
        count: migration.retired,
        scope: "baseline policy1 receipt documents",
        attemptedDemandRefusals: sumNamedCounts(
          attempts,
          "refusalCountsByOwner",
          "probe-gate"
        ),
        note: "exact retirement owner; attempted demand refusals are reported separately"
      },
      {
        category: "resource-limit",
        count: resourceLimitRefusals.length,
        scope: "ecosystem proposal artifact-case censuses",
        note: "finite surfaces beyond the Rust-owned candidate budget remain explicit local refusals"
      },
      {
        category: "trust",
        count: attemptOwners.trust ?? 0,
        scope: "policy2 certification attempts",
        note: "issuance was not reached"
      },
      {
        category: "authentication",
        count: migration.retired,
        scope: "obsolete policy1 receipt documents",
        note: "policy1 cannot authenticate policy2 analyzer semantics"
      },
      {
        category: "semantic",
        count: content.unknownTotal,
        scope: "open claim leaves in generatable ecosystem proposals",
        note: "open is not negative proof"
      }
    ],
    demandEvidence: {
      denominatorMeaning:
        "authority-demand definitions plus exact demand instances derived for structurally complete rows",
      families: demandFamilies(demandAuthority, attempts)
    },
    costs: {
      proofInput: observedDistribution(
        attempts.map(attempt =>
          (attempt.stageDurationsMs?.proposalGeneration ?? Number.NaN) +
          (attempt.stageDurationsMs?.demandPlanning ?? Number.NaN)
        ),
        "milliseconds per policy2 certification attempt",
        "not-observed-no-policy2-certification-attempt"
      ),
      verification: observedDistribution(
        attempts.map(attempt => attempt.stageDurationsMs?.certification ?? Number.NaN),
        "milliseconds per policy2 certification attempt",
        "not-observed-no-attempt-reached-certification"
      ),
      receipt: observedDistribution(
        attempts.map(attempt => attempt.stageDurationsMs?.receiptIssuance ?? Number.NaN),
        "milliseconds per policy2 receipt issuance",
        "not-observed-no-policy2-receipt-issued"
      ),
      load: unobservedDistribution(
        "nanoseconds per accepted policy2 catalog load",
        "not-observed-no-active-policy2-receipt"
      ),
      query: unobservedDistribution(
        "nanoseconds per accepted policy2 export query",
        "not-observed-no-active-policy2-receipt"
      )
    },
    byteDistributions: {
      proposalArtifacts: {
        main: content.wireBytes.canonicalMain,
        demandPlan: content.wireBytes.proposalPlan,
        authorityStatus: "open-proposal-not-accepted-policy2-artifact"
      },
      acceptedPolicy2Artifacts: {
        main: unobservedDistribution(
          "bytes",
          "not-observed-no-active-policy2-receipt"
        ),
        proof: unobservedDistribution(
          "bytes",
          "not-observed-no-attempt-reached-certification"
        ),
        sidecar: unobservedDistribution(
          "bytes",
          "not-observed-no-attempt-reached-certification"
        ),
        receipt: unobservedDistribution(
          "bytes",
          "not-observed-no-policy2-receipt-issued"
        )
      }
    },
    ordinaryAnalysis: {
      networkAccess: false,
      packageCodeExecution: false,
      rawEvidenceBytes: 0,
      acceptsAuditTranscript: false,
      acceptsOpenProposal: false
    }
  };
}

function renderMarkdown(report) {
  const status = report.artifactStatus;
  const issuance = report.policy2Issuance;
  return `# Phase 19 authenticated policy and refusal-leverage report

- Ecosystem rows: ${status.denominator} (${status.completeProposal} complete proposals, ${status.partialProposal} partial, ${status.fullRefusal} full refusals, ${status.notApplicable} not applicable)
- Policy-2 receipts: ${issuance.activeReceiptDocuments.issued}/${issuance.activeReceiptDocuments.denominator} baseline receipt documents
- Policy-2 verified exports: ${report.verifiedSemantics.policy2VerifiedExports}/${report.verifiedSemantics.proposalExports} proposal exports
- Policy-1 migration: ${report.policyStrengthRefusals.reissued} reissued, ${report.policyStrengthRefusals.retiredOrDemoted} retired/demoted, ${report.policyStrengthRefusals.pending} pending
- Structurally complete rows attempted: ${report.certificationAttempts.attempted}/${report.certificationAttempts.structurallyCompleteRows}

No acceptance target weakens proof. The current zero issuance count is a result: the mandatory live producer and probe authorities are incomplete, so policy 2 cannot authenticate any ecosystem row yet.

## Refusal owner queue

| Order | Owner | Rows | Disposition |
| ---: | --- | ---: | --- |
${report.refusalOwnerQueue.map(row => `| ${row.order} | ${row.owner} | ${row.rows} | ${row.disposition} |`).join("\n")}

Finite wildcard census support was remeasured across the 418-row corpus. Its five historical rows now expose deeper exact refusals and unlock zero verified exports; the eight no-ESM rows remain refusals.

## Measurement availability

Open proposal main bytes are measured (${report.byteDistributions.proposalArtifacts.main.count} samples), and proof-input cost is measured for ${report.costs.proofInput.count} snapshot-bound certification attempts. Verification, receipt, accepted-load, accepted-query, accepted main, proof, sidecar, and receipt distributions retain zero samples where the exact missing live authority stopped the transaction; null percentiles are reported instead of fabricated zero costs.

## Trust boundary

Ordinary analysis consumes no audit transcript, open proposal, raw evidence, registry response, or package execution. Every active policy-2 count remains zero until an authenticated receipt closes the exact demand graph.
`;
}

function loadSources() {
  return {
    ecosystem: json(inputs.ecosystem),
    phase16Refusals: json(inputs.phase16Refusals),
    baseline: json(inputs.baseline),
    migration: json(inputs.migration),
    demandAuthority: json(inputs.demandAuthority),
    policy: json(inputs.policy)
  };
}

export function assertPhase19Report(report, sources = loadSources()) {
  assert.deepEqual(report, buildPhase19Report(sources));
  assert.equal(report.policyStrengthRefusals.pending, 0);
  assert.equal(report.verifiedExportLeverage.totalUnlocked, 0);
  assert.equal(
    report.demandEvidence.families.reduce(
      (total, family) => total + family.authenticatedEvidenceBytes,
      0
    ),
    0
  );
}

function main() {
  const [mode] = process.argv.slice(2);
  const sources = loadSources();
  if (mode === "--write") {
    const report = buildPhase19Report(sources);
    mkdirSync(phase19, { recursive: true });
    writeFileSync(reportPath, `${JSON.stringify(report, null, 2)}\n`);
    writeFileSync(markdownPath, renderMarkdown(report));
    console.log(`wrote Phase 19 report for ${report.artifactStatus.denominator} rows`);
    return;
  }
  if (mode === "--check") {
    const report = JSON.parse(readFileSync(reportPath, "utf8"));
    assertPhase19Report(report, sources);
    assert.equal(readFileSync(markdownPath, "utf8"), renderMarkdown(report));
    console.log(
      `checked Phase 19: ${report.policy2Issuance.activeReceiptDocuments.issued} policy-2 receipts, ${report.policyStrengthRefusals.pending} pending migrations`
    );
    return;
  }
  throw new Error(
    "usage: bun scripts/package-contract-v2-phase19-report.mjs --write | --check"
  );
}

if (import.meta.main) main();
