import assert from "node:assert/strict";
import { describe, test } from "vitest";

import {
  auditPhase19Baseline,
  auditPhase19BaselineArtifact,
  auditPhase19DemandAuthority,
  auditPhase19Policy
} from "./package-contract-phase19.mjs";

describe("Phase 19 authenticated proof-policy baseline", () => {
  test("freezes the policy-1 authority and refusal envelope before replacement", () => {
    assert.deepEqual(auditPhase19Baseline(), {
      stableMainDocuments: 130,
      activeReceiptDocuments: 73,
      receiptIssuedArtifactCases: 24,
      ecosystemRows: 418,
      completeProposals: 40,
      partialProposals: 318,
      fullRowRefusals: 60,
      artifactCaseLocalRefusals: 1458,
      refusalOwnerCounts: {
        generatedFailures: 60,
        partialEntrypoints: 0,
        partialArtifactCases: 1458,
        locallyOpenClaimDomains: 10,
        conformanceOpenRows: 36
      },
      proofVersion: 1,
      proofPolicy: 1,
      receiptVersion: 1,
      typeFactsProducer: {
        sourceManifest: "sha256:aa653630f88754304bf4d6722859a4e686f833340f2963e7665b1a308de1e793",
        handshakeProtocol: 3,
        schema: "sha256:b071a78a86949a1e4162408912d7622aed0460fba3a64fd52506fc14091417c7",
        buildId: "dev"
      },
      compilerProducers: [
        "solid-v1:trace2:ca3bbfae7d1e00e28ef73f9af58bdb46e248b512",
        "solid-v2:trace3:7f4e1135943c1fb01231d1bda707b4a1856a5607"
      ],
      checkedCorpusShortcutOwners: [
        "rust/crates/solid-facts-backend/src/contract_workflow.rs"
      ],
      callerProofIssuanceOwners: [
        "packages/cli/scripts/verify-contract.mjs"
      ]
    });
  });

  test("pins the internal policy-2 audit rendering and golden digest", () => {
    assert.deepEqual(auditPhase19Policy(), {
      policyVersion: 2,
      proofVersion: 2,
      receiptVersion: 2,
      semanticModelVersion: 1,
      status: "internal-not-active",
      artifactPrerequisiteFamilies: 6,
      claimFamilies: 9,
      policyDigest:
        "sha256:aeea7aaaa8ee5a85946328719b66e8ed185c38e7989da19e26d0424bb743e4db"
    });
  });

  test("keeps the checked-in baseline inventory equal to the executable audit", () => {
    assert.deepEqual(auditPhase19BaselineArtifact(), auditPhase19Baseline());
  });

  test("classifies every policy-2 demand against an existing producer guarantee", () => {
    assert.deepEqual(auditPhase19DemandAuthority(), {
      demands: 43,
      families: 18,
      alreadyExact: 23,
      producerExtensionRequired: 13,
      unsupported: 7,
      certificationReadyFamilies: 4
    });
  });
});
