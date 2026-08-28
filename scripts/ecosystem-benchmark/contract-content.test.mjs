import assert from "node:assert/strict";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "vitest";

import {
  BEHAVIORAL_ROW_KINDS,
  CLAIM_DOMAINS,
  isUnknownClaim,
  readContractContent,
  reviewPlanPathFor,
  summarizeContract,
  summarizeContractDocument,
  summarizeReviewPlan
} from "./lib/contract-content.mjs";

function document() {
  return {
    format: "solid-reactivity-contract",
    schemaVersion: 2,
    semanticModelVersion: 1,
    package: { name: "pkg", version: "1.0.0", integrity: "fixture" },
    summaries: {
      closed: {
        shape: "callable",
        call: {
          closed: CLAIM_DOMAINS.filter(domain => domain !== "recursiveValue"),
          operations: [{ id: "read", kind: "read" }]
        }
      },
      partial: { shape: { kind: "unknown" }, call: { closed: ["callbacks"] } }
    },
    entrypoints: {
      ".": {
        cases: [
          { exports: { closed: "closed", partial: { summary: "partial" } } }
        ]
      }
    }
  };
}

function plan() {
  return {
    format: "solid-checker-contract-proposal-plan",
    planVersion: 1,
    closureCandidates: [{ claimId: "one" }],
    proofCandidates: [{ claimId: "two" }],
    probeCandidates: []
  };
}

test("temporary-v2 unknown value leaves use the model vocabulary, not legacy sentinels", () => {
  assert.equal(isUnknownClaim("unknown"), true);
  assert.equal(isUnknownClaim({ kind: "unknown" }), true);
  assert.equal(isUnknownClaim({ status: "unknown" }), false);
});

test("wire measurement counts export names, open domains, and operation kinds", () => {
  const summary = summarizeContractDocument(document());
  assert.equal(summary.exportsTotal, 2);
  assert.equal(summary.exportsProven, 1);
  assert.equal(summary.exportsWithUnknown, 1);
  assert.equal(summary.unknownByDomain.callbacks, 0);
  assert.equal(summary.unknownByDomain.reads, 1);
  assert.equal(summary.unknownByDomain.recursiveValue, 1);
  assert.equal(summary.behavioralRows.read, 1);
  assert.deepEqual(Object.keys(summary.behavioralRows), BEHAVIORAL_ROW_KINDS);
});

test("legacy schema-version-1 documents are not measured", () => {
  assert.equal(summarizeContractDocument({ ...document(), schemaVersion: 1 }), null);
});

test("proposal plans remain distinct from proof acceptance", () => {
  assert.equal(summarizeReviewPlan(plan()).checklistItems, 2);
  assert.equal(summarizeContract({ contract: document(), reviewPlan: plan() }).fullyProven, false);
});

test("readContractContent uses the proposal-plan sibling", () => {
  const root = mkdtempSync(join(tmpdir(), "solid-checker-content-v2-"));
  const path = join(root, "solid-reactivity.json");
  try {
    writeFileSync(path, JSON.stringify(document()));
    writeFileSync(reviewPlanPathFor(path), JSON.stringify(plan()));
    const content = readContractContent(path, 0);
    assert.equal(content.measured, true);
    assert.equal(content.reviewPlanItems, 2);
    assert.equal(content.fullyProven, false);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
