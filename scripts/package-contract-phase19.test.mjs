import assert from "node:assert/strict";
import { describe, test } from "vitest";

import {
  auditPhase19Baseline,
  auditPhase19BaselineArtifact,
  auditPhase19Cut,
  auditPhase19DemandAuthority,
  auditPhase19Policy
} from "./package-contract-phase19.mjs";

describe("Phase 19 authenticated proof-policy baseline", () => {
  test("retains the immutable policy-1 baseline as historical evidence", () => {
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
        sourceManifest: "sha256:12a0585d59618a2e227f1f25fba613c4a1c5bddc7c6f17c401616f8194e8d10a",
        handshakeProtocol: 4,
        schema: "sha256:129f78430a829013b3fe1a6fd9948b27f7ba7269858dd8438e61d5b2bef76fbe",
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

  test("pins the active policy-2 audit rendering and golden digest", () => {
    assert.deepEqual(auditPhase19Policy(), {
      policyVersion: 2,
      proofVersion: 2,
      receiptVersion: 2,
      semanticModelVersion: 1,
      status: "active",
      artifactPrerequisiteFamilies: 6,
      claimFamilies: 9,
      policyDigest:
        "sha256:f0dfd235055d1aba95f1de513eeee8109178a186fb2be3901d1f3092a42bb278"
    });
  });

  test("reads the historical baseline without reinterpreting current source", () => {
    assert.deepEqual(auditPhase19BaselineArtifact(), auditPhase19Baseline());
  });

  test("closes every baseline receipt row in the policy-2 atomic cut", () => {
    assert.deepEqual(auditPhase19Cut(), {
      // The cut audit enumerates tracked *.json via git ls-files, so this pin
      // moves whenever a fixture main document becomes tracked. 130 was
      // measured while three certifying fixtures (class-expression-kind,
      // escaping-private-helper, published-export-entity) were still
      // untracked and invisible to the audit. 141 includes the generated
      // Type Facts implementation-transcript main document added by Phase 21.
      // 143 adds the destructured-parameter-callback generator fixture's main
      // document. 163 adds the twenty main documents pinned when the orphaned
      // package-contract fixtures were registered in the generator corpus --
      // twenty rather than twenty-three, because legacy-cjs-entrypoint,
      // external-reexport and solid-reexport are fail-closed refusals and pin
      // an expected-refusal.txt instead of a main document. 167 adds the four
      // fixtures of the invoking-position round: callback-slot-derived-store,
      // callback-slot-derived-store-server, callback-slot-props-forwarding,
      // and parameter-member-read-path. 168 adds the exact shared runtime and
      // declaration surface pinned by torture-dts-disagreement.
      stableMainDocuments: 168,
      activePolicy2Receipts: 0,
      activePolicy1Receipts: 0,
      baselineReceipts: 73,
      reissuedReceipts: 0,
      retiredReceipts: 73,
      pendingReceipts: 0,
      checkedCorpusShortcuts: 0,
      callerProofIssuancePaths: 0,
      automaticCertificationWorkflows: 1,
      obsoletePolicy1Catalogs: 21,
      proofVersion: 2,
      receiptVersion: 2,
      policyStatus: "active",
      policyDigest:
        "sha256:f0dfd235055d1aba95f1de513eeee8109178a186fb2be3901d1f3092a42bb278"
    });
  });

  test("classifies every policy-2 demand against an existing producer guarantee", () => {
    assert.deepEqual(auditPhase19DemandAuthority(), {
      demands: 43,
      families: 18,
      alreadyExact: 31,
      producerExtensionRequired: 5,
      unsupported: 7,
      certificationReadyFamilies: 7
    });
  });
});
