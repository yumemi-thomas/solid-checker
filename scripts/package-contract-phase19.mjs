#!/usr/bin/env bun

// Phase 19A baseline authority.
//
// This inventory deliberately describes the policy-1 state that Phase 19A
// must replace atomically. It is not a policy-2 verifier and cannot issue or
// accept package contracts.

import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { isDeepStrictEqual } from "node:util";

import { auditRepository as auditPhase18Repository } from "./package-contract-phase18.mjs";
import { sourceDigest as typeFactsSourceDigest } from "./typefacts-source-identity.mjs";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");

const PATHS = Object.freeze({
  phase16Report: "benchmarks/package-contract-v2/phase16/report.json",
  baselineInventory: "docs/package-contract-v2/phase19/policy1-baseline.json",
  policy2Manifest: "docs/package-contract-v2/phase19/proof-policy-v2.json",
  demandAuthorityAudit:
    "docs/package-contract-v2/phase19/proof-demand-authority-audit.json",
  proofAuthority: "rust/crates/solid-reactive-ir/src/contract_semantics/proof.rs",
  checkedCorpusAuthority: "rust/crates/solid-facts-backend/src/contract_workflow.rs",
  callerProofAuthority: "packages/cli/scripts/verify-contract.mjs",
  typeFactsProtocol: "rust/crates/typefacts/src/v3.rs",
  solidV1Compiler: "rust/dialects/solid-v1/compiler/src/lib.rs",
  solidV2Compiler: "rust/dialects/solid-v2/compiler/src/lib.rs"
});

function readText(root, path) {
  return readFileSync(join(root, path), "utf8");
}

function readJson(root, path) {
  try {
    return JSON.parse(readText(root, path));
  } catch (error) {
    throw new Error(`${path} is not valid JSON: ${error.message}`);
  }
}

function requireIntegerConstant(source, path, name) {
  const match = source.match(new RegExp(`\\b${name}: u(?:16|32|64) = (\\d+);`));
  if (!match) throw new Error(`${path} is missing integer constant ${name}`);
  return Number.parseInt(match[1], 10);
}

function requireStringConstant(source, path, name) {
  const match = source.match(new RegExp(`\\b${name}: &str =\\s*"([^"]+)";`));
  if (!match) throw new Error(`${path} is missing string constant ${name}`);
  return match[1];
}

function requireDefaultBuildId(source, path) {
  const match = source.match(
    /pub const TYPE_FACTS_BUILD_ID: &str = match[\s\S]*?None => "([^"]+)",/
  );
  if (!match) throw new Error(`${path} is missing the default Type Facts build ID`);
  return match[1];
}

function requireMarkers(source, path, markers) {
  for (const marker of markers) {
    if (!source.includes(marker)) throw new Error(`${path} is missing baseline marker ${marker}`);
  }
  return path;
}

function u64be(value) {
  const bytes = Buffer.alloc(8);
  bytes.writeBigUInt64BE(BigInt(value));
  return bytes;
}

export function auditPhase19Policy(root = repositoryRoot) {
  const policy = readJson(root, PATHS.policy2Manifest);
  if (policy.format !== "solid-checker-contract-proof-policy") {
    throw new Error(`${PATHS.policy2Manifest} has the wrong format`);
  }
  const digestRule = policy.digests?.find(rule => rule.purpose === "policy");
  if (
    digestRule?.algorithm !== "sha256" ||
    digestRule?.domain !== "solid-checker:contract-proof-policy:v2" ||
    digestRule?.framing !== "u64be-length-prefixed-domain-and-payload"
  ) {
    throw new Error(`${PATHS.policy2Manifest} has an unsupported policy digest rule`);
  }
  const canonical = Buffer.from(JSON.stringify(policy));
  const domain = Buffer.from(digestRule.domain);
  const digest = createHash("sha256")
    .update(u64be(domain.length))
    .update(domain)
    .update(u64be(canonical.length))
    .update(canonical)
    .digest("hex");

  return {
    policyVersion: policy.policyVersion,
    proofVersion: policy.proofVersion,
    receiptVersion: policy.receiptVersion,
    semanticModelVersion: policy.semanticModelVersion,
    status: policy.status,
    artifactPrerequisiteFamilies: policy.applicability?.artifactPrerequisites?.length,
    claimFamilies: policy.applicability?.claimFamilies?.length,
    policyDigest: `sha256:${digest}`
  };
}

function policyFamilies(policy) {
  const applicability = policy.applicability;
  return new Set([
    ...(applicability?.artifactPrerequisites ?? []).map(rule => rule.family),
    ...(applicability?.claimFamilies ?? []).map(rule => rule.family),
    applicability?.compilerReconciliation?.family,
    applicability?.dependencyComposition?.family,
    applicability?.probeConsistency?.family
  ]);
}

export function auditPhase19DemandAuthority(root = repositoryRoot) {
  const policy = readJson(root, PATHS.policy2Manifest);
  const audit = readJson(root, PATHS.demandAuthorityAudit);
  if (
    audit.schemaVersion !== 1 ||
    audit.documentKind !== "solid-checker-package-contract-phase19-demand-authority-audit" ||
    audit.capturedAt !== "2026-08-30" ||
    audit.certificationReady !== false ||
    !Array.isArray(audit.demands)
  ) {
    throw new Error(`${PATHS.demandAuthorityAudit} has an invalid audit envelope`);
  }

  const families = policyFamilies(policy);
  if (families.has(undefined)) {
    throw new Error(`${PATHS.policy2Manifest} has an incomplete applicability table`);
  }
  const representedFamilies = new Map([...families].map(family => [family, []]));
  const ids = new Set();
  const counts = {
    "already exact": 0,
    "producer extension required": 0,
    unsupported: 0
  };

  for (const demand of audit.demands) {
    if (typeof demand.id !== "string" || !/^[a-z0-9-]+\/[a-z0-9-]+$/.test(demand.id)) {
      throw new Error(`${PATHS.demandAuthorityAudit} has a noncanonical demand ID`);
    }
    if (!ids.add(demand.id)) {
      throw new Error(`${PATHS.demandAuthorityAudit} repeats demand ${demand.id}`);
    }
    const familyDemands = representedFamilies.get(demand.family);
    if (!familyDemands) {
      throw new Error(`${PATHS.demandAuthorityAudit} names unknown family ${demand.family}`);
    }
    if (!Object.hasOwn(counts, demand.status)) {
      throw new Error(`${PATHS.demandAuthorityAudit} has unknown status ${demand.status}`);
    }
    if (
      typeof demand.source !== "string" ||
      demand.source.startsWith("/") ||
      demand.source.split("/").includes("..") ||
      !existsSync(join(root, demand.source))
    ) {
      throw new Error(`${PATHS.demandAuthorityAudit} names missing source ${demand.source}`);
    }
    if (
      typeof demand.completenessGuarantee !== "string" ||
      demand.completenessGuarantee.length === 0 ||
      typeof demand.policy2Gap !== "string" ||
      demand.policy2Gap.length === 0
    ) {
      throw new Error(`${PATHS.demandAuthorityAudit} leaves demand ${demand.id} unexplained`);
    }
    counts[demand.status] += 1;
    familyDemands.push(demand);
  }

  for (const [family, demands] of representedFamilies) {
    if (demands.length === 0) {
      throw new Error(`${PATHS.demandAuthorityAudit} does not audit policy family ${family}`);
    }
  }
  const certificationReadyFamilies = [...representedFamilies.values()].filter(demands =>
    demands.every(demand => demand.status === "already exact")
  ).length;

  return {
    demands: audit.demands.length,
    families: representedFamilies.size,
    alreadyExact: counts["already exact"],
    producerExtensionRequired: counts["producer extension required"],
    unsupported: counts.unsupported,
    certificationReadyFamilies
  };
}

export function auditPhase19BaselineArtifact(root = repositoryRoot) {
  const artifact = readJson(root, PATHS.baselineInventory);
  if (
    artifact.schemaVersion !== 1 ||
    artifact.documentKind !== "solid-checker-package-contract-phase19-policy1-baseline" ||
    artifact.capturedAt !== "2026-08-29" ||
    !artifact.inventory
  ) {
    throw new Error(`${PATHS.baselineInventory} has an invalid baseline envelope`);
  }
  return artifact.inventory;
}

export function auditPhase19Baseline(root = repositoryRoot) {
  const phase18 = auditPhase18Repository(root);
  const phase16 = readJson(root, PATHS.phase16Report);
  const proofAuthority = readText(root, PATHS.proofAuthority);
  const checkedCorpusAuthority = readText(root, PATHS.checkedCorpusAuthority);
  const callerProofAuthority = readText(root, PATHS.callerProofAuthority);
  const typeFactsProtocol = readText(root, PATHS.typeFactsProtocol);
  const solidV1Compiler = readText(root, PATHS.solidV1Compiler);
  const solidV2Compiler = readText(root, PATHS.solidV2Compiler);

  const corpus = phase16?.corpus;
  if (!corpus?.ecosystem || !corpus?.refusalCounts) {
    throw new Error(`${PATHS.phase16Report} is missing the Phase 19 baseline corpus envelope`);
  }

  return {
    stableMainDocuments: phase18.mainDocuments,
    activeReceiptDocuments: phase18.receipts,
    receiptIssuedArtifactCases: corpus.preservedReceiptIssuedRows,
    ecosystemRows: corpus.ecosystem.rows,
    completeProposals: corpus.ecosystem.complete,
    partialProposals: corpus.ecosystem.partial,
    fullRowRefusals: corpus.ecosystem.refused,
    artifactCaseLocalRefusals: corpus.refusalCounts.partialArtifactCases,
    refusalOwnerCounts: {
      generatedFailures: corpus.refusalCounts.generatedFailures,
      partialEntrypoints: corpus.refusalCounts.partialEntrypoints,
      partialArtifactCases: corpus.refusalCounts.partialArtifactCases,
      locallyOpenClaimDomains: corpus.refusalCounts.locallyOpenClaimDomains,
      conformanceOpenRows: corpus.refusalCounts.conformanceOpenRows
    },
    proofVersion: requireIntegerConstant(
      checkedCorpusAuthority,
      PATHS.checkedCorpusAuthority,
      "PROOF_VERSION"
    ),
    proofPolicy: requireIntegerConstant(
      proofAuthority,
      PATHS.proofAuthority,
      "PROOF_POLICY_VERSION"
    ),
    receiptVersion: requireIntegerConstant(
      proofAuthority,
      PATHS.proofAuthority,
      "ACCEPTANCE_RECEIPT_VERSION"
    ),
    typeFactsProducer: {
      sourceManifest: `sha256:${typeFactsSourceDigest(root)}`,
      handshakeProtocol: requireIntegerConstant(
        typeFactsProtocol,
        PATHS.typeFactsProtocol,
        "TYPE_FACTS_HANDSHAKE_PROTOCOL"
      ),
      schema: requireStringConstant(
        typeFactsProtocol,
        PATHS.typeFactsProtocol,
        "TYPE_FACTS_SCHEMA_SHA256"
      ),
      buildId: requireDefaultBuildId(typeFactsProtocol, PATHS.typeFactsProtocol)
    },
    compilerProducers: [
      requireStringConstant(solidV1Compiler, PATHS.solidV1Compiler, "COMPILER_FACTS_IDENTITY"),
      requireStringConstant(solidV2Compiler, PATHS.solidV2Compiler, "COMPILER_FACTS_IDENTITY")
    ],
    checkedCorpusShortcutOwners: [
      requireMarkers(checkedCorpusAuthority, PATHS.checkedCorpusAuthority, [
        "fn accept_checked_corpus_case(",
        "complete: true,",
        "enumerated: vec![census.clone()]",
        "classified: vec![census.clone()]"
      ])
    ],
    callerProofIssuanceOwners: [
      requireMarkers(callerProofAuthority, PATHS.callerProofAuthority, [
        "--proof <FILE>",
        '"--verify-proof"',
        "receipt bytes"
      ])
    ]
  };
}

function main() {
  const baseline = auditPhase19Baseline();
  const frozenBaseline = auditPhase19BaselineArtifact();
  if (!isDeepStrictEqual(baseline, frozenBaseline)) {
    throw new Error(`${PATHS.baselineInventory} has drifted from the executable policy-1 audit`);
  }
  const policy = auditPhase19Policy();
  const demandAuthority = auditPhase19DemandAuthority();
  console.log(
    `phase19 baseline: ${baseline.stableMainDocuments} stable-v1 mains, ` +
      `${baseline.activeReceiptDocuments} policy-1 receipts, ` +
      `${baseline.receiptIssuedArtifactCases} receipt-issued artifact cases; ` +
      `${baseline.ecosystemRows} ecosystem rows ` +
      `(${baseline.completeProposals} complete / ${baseline.partialProposals} partial / ` +
      `${baseline.fullRowRefusals} refused), ` +
      `${baseline.artifactCaseLocalRefusals} artifact-case-local refusals; ` +
      `${baseline.checkedCorpusShortcutOwners.length + baseline.callerProofIssuanceOwners.length} ` +
      "policy-1 issuance shortcuts frozen; " +
      `internal policy ${policy.policyVersion} ${policy.policyDigest}; ` +
      `${demandAuthority.demands} authority demands audited ` +
      `(${demandAuthority.alreadyExact} exact / ` +
      `${demandAuthority.producerExtensionRequired} extensions / ` +
      `${demandAuthority.unsupported} unsupported)`
  );
}

if (import.meta.main) main();
