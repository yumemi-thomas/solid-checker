#!/usr/bin/env bun

// Phase 19A baseline authority.
//
// This inventory deliberately describes the policy-1 state that Phase 19A
// must replace atomically. It is not a policy-2 verifier and cannot issue or
// accept package contracts.

import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { auditRepository as auditPhase18Repository } from "./package-contract-phase18.mjs";
import { sourceDigest as typeFactsSourceDigest } from "./typefacts-source-identity.mjs";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");

const PATHS = Object.freeze({
  phase16Report: "benchmarks/package-contract-v2/phase16/report.json",
  baselineInventory: "docs/package-contract-v2/phase19/policy1-baseline.json",
  policy2Manifest: "docs/package-contract-v2/phase19/proof-policy-v2.json",
  demandAuthorityAudit:
    "docs/package-contract-v2/phase19/proof-demand-authority-audit.json",
  receiptMigration: "docs/package-contract-v2/phase19/receipt-migration.json",
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
  return auditPhase19BaselineArtifact(root);
}

export function auditPhase19Cut(root = repositoryRoot) {
  const phase18 = auditPhase18Repository(root);
  const policy = auditPhase19Policy(root);
  const migration = readJson(root, PATHS.receiptMigration);
  if (
    migration.schemaVersion !== 1 ||
    migration.documentKind !== "solid-checker-package-contract-phase19-receipt-migration" ||
    migration.cutAt !== "2026-08-30" ||
    !Array.isArray(migration.rows)
  ) {
    throw new Error(`${PATHS.receiptMigration} has an invalid migration envelope`);
  }
  const paths = new Set();
  const counts = { reissued: 0, retired: 0, pending: 0 };
  for (const row of migration.rows) {
    if (
      typeof row.path !== "string" ||
      !row.path.endsWith(".receipt.json") ||
      paths.has(row.path) ||
      !Object.hasOwn(counts, row.status)
    ) {
      throw new Error(`${PATHS.receiptMigration} has an invalid or duplicate row`);
    }
    paths.add(row.path);
    counts[row.status] += 1;
    if (row.status === "retired") {
      if (existsSync(join(root, row.path))) {
        throw new Error(`${row.path} is retired but remains active`);
      }
      if (row.refusalOwner !== "probe-gate" || !row.refusal?.includes("harness binding")) {
        throw new Error(`${row.path} has no exact retirement refusal`);
      }
    }
  }
  if (
    migration.baselineReceipts !== 73 ||
    migration.rows.length !== migration.baselineReceipts ||
    migration.reissued !== counts.reissued ||
    migration.retired !== counts.retired ||
    migration.pending !== counts.pending ||
    counts.pending !== 0
  ) {
    throw new Error(`${PATHS.receiptMigration} counts do not close the 73-receipt baseline`);
  }

  const checkedCorpus = readText(root, PATHS.checkedCorpusAuthority);
  const callerProof = readText(root, PATHS.callerProofAuthority);
  const native = readText(root, "rust/crates/solid-facts-backend/src/main.rs");
  for (const [path, source, marker] of [
    [PATHS.checkedCorpusAuthority, checkedCorpus, "accept_checked_corpus_case"],
    [PATHS.callerProofAuthority, callerProof, "--proof <FILE>"],
    ["rust/crates/solid-facts-backend/src/main.rs", native, '"--verify-proof"']
  ]) {
    if (source.includes(marker)) throw new Error(`${path} retains policy-1 shortcut ${marker}`);
  }
  if (existsSync(join(root, "rust/crates/solid-facts-backend/src/proof_checker.rs"))) {
    throw new Error("retired policy-1 proof checker remains active");
  }
  if (policy.status !== "active" || policy.proofVersion !== 2 || policy.receiptVersion !== 2) {
    throw new Error("policy 2 is not the active proof/receipt authority");
  }

  const refusalCatalogs = execFileSync(
    "git",
    ["ls-files", "-z", "--", "*/.solid-checker/accepted-contracts.json"],
    { cwd: root, encoding: "utf8" }
  )
    .split("\0")
    .filter(path => path && existsSync(join(root, path)));
  for (const path of refusalCatalogs) {
    const catalog = readJson(root, path);
    if (
      catalog.format !== "solid-checker-accepted-contract-catalog" ||
      catalog.catalogVersion !== 2 ||
      !catalog.contracts?.every(entry => entry.status === "obsolete-policy1" && !("receipt" in entry))
    ) {
      throw new Error(`${path} is not a policy-1 refusal-only catalog`);
    }
  }

  return {
    stableMainDocuments: phase18.mainDocuments,
    activePolicy2Receipts: phase18.receipts,
    activePolicy1Receipts: 0,
    baselineReceipts: migration.baselineReceipts,
    reissuedReceipts: counts.reissued,
    retiredReceipts: counts.retired,
    pendingReceipts: counts.pending,
    checkedCorpusShortcuts: 0,
    callerProofIssuancePaths: 0,
    obsoletePolicy1Catalogs: refusalCatalogs.length,
    proofVersion: policy.proofVersion,
    receiptVersion: policy.receiptVersion,
    policyStatus: policy.status,
    policyDigest: policy.policyDigest
  };
}

function main() {
  const baseline = auditPhase19Baseline();
  const policy = auditPhase19Policy();
  const demandAuthority = auditPhase19DemandAuthority();
  const cut = auditPhase19Cut();
  console.log(
    `phase19 cut: ${cut.stableMainDocuments} stable-v1 mains, ` +
      `${cut.activePolicy1Receipts} policy-1 receipts, ` +
      `${cut.activePolicy2Receipts} policy-2 receipts; ` +
      `${cut.retiredReceipts} retired / ${cut.reissuedReceipts} reissued / ` +
      `${cut.pendingReceipts} pending from ${baseline.activeReceiptDocuments}; ` +
      `active policy ${policy.policyVersion} ${policy.policyDigest}; ` +
      `${demandAuthority.demands} authority demands audited ` +
      `(${demandAuthority.alreadyExact} exact / ` +
      `${demandAuthority.producerExtensionRequired} extensions / ` +
      `${demandAuthority.unsupported} unsupported)`
  );
}

if (import.meta.main) main();
