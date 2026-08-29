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

function demandFamilies(audit) {
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
        observedCertificationDemands: 0,
        authenticatedEvidenceBytes: 0,
        observationStatus: "not-attempted-no-complete-live-producer-set"
      };
    });
}

export function buildPhase19Report(sources) {
  const { ecosystem, phase16Refusals, baseline, migration, demandAuthority } = sources;
  const official = ecosystem.results.filter(result => result.status !== "supplemental");
  const status = countsBy(official, "outcome");
  const content = ecosystem.combined.contractContent;
  const failureClasses = countsBy(phase16Refusals.generatedProposalFailures, "class");
  const finiteWildcardRows = phase16Refusals.generatedProposalFailures.filter(
    row =>
      row.class === "unclassified" &&
      (/wildcard export/.test(row.reason) || /package exports .*\*/.test(row.reason))
  ).length;
  const cut = auditPhase19Cut(root);
  const authority = auditPhase19DemandAuthority(root);

  assert.equal(official.length, 418);
  assert.equal(status.success, 40);
  assert.equal(status["partial-success"], 318);
  assert.equal(status.failure, 60);
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
      attempted: 0,
      newlyCertified: 0,
      reason:
        "no complete authenticated live producer set exists; running a partial attempt cannot issue authority"
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
        historicalRows: finiteWildcardRows,
        otherArtifactShapeRows:
          (failureClasses.unclassified ?? 0) - finiteWildcardRows,
        implementationStatus: "implemented-remeasurement-pending",
        verifiedExportsUnlocked: 0
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
          "finite-wildcard-subset-implemented-remeasurement-pending; other artifact shapes open"
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
        count: 0,
        scope: "policy2 certification attempts",
        note: "no complete-row certification attempt was started"
      },
      {
        category: "producer-session",
        count: authority.producerExtensionRequired + authority.unsupported,
        scope: "policy2 authority-demand definitions",
        note: "producer gaps; not multiplied by ecosystem rows"
      },
      {
        category: "probe-gate",
        count: migration.retired,
        scope: "baseline policy1 receipt documents",
        note: "exact retirement owner"
      },
      {
        category: "resource-limit",
        count: 0,
        scope: "policy2 certification attempts",
        note: "no observed attempt-level resource refusal"
      },
      {
        category: "trust",
        count: 0,
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
      denominatorMeaning: "authority-demand definitions, not ecosystem demand instances",
      families: demandFamilies(demandAuthority)
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

Finite wildcard census support is implemented but deliberately reports zero unlocked exports until the 418-row corpus is rerun. The eight no-ESM rows remain refusals.

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
