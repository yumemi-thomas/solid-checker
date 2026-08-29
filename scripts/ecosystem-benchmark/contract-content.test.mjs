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
  refusalPathFor,
  reviewPlanPathFor,
  summarizeContract,
  summarizeContractDocument,
  summarizeReviewPlan
} from "./lib/contract-content.mjs";

function document() {
  return {
    format: "solid-reactivity-contract",
    schemaVersion: 1,
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

test("stable-v1 unknown value leaves use the model vocabulary, not legacy sentinels", () => {
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

test("retired temporary schema-version-2 documents are not measured", () => {
  assert.equal(summarizeContractDocument({ ...document(), schemaVersion: 2 }), null);
});

test("proposal plans remain distinct from proof acceptance", () => {
  assert.equal(summarizeReviewPlan(plan()).checklistItems, 2);
  const summary = summarizeContract({ contract: document(), reviewPlan: plan() });
  assert.equal(summary.fullyProven, false);
  assert.equal(summary.wireBytes.canonicalMain, Buffer.byteLength(`${JSON.stringify(document())}\n`));
  assert.equal(summary.wireBytes.proposalPlan, null);
  assert.ok(summary.wireBytes.perExport > 0);
  assert.ok(summary.wireBytes.perOperation > 0);
});

test("readContractContent uses the proposal-plan sibling", () => {
  const root = mkdtempSync(join(tmpdir(), "solid-checker-content-v2-"));
  const path = join(root, "solid-reactivity.json");
  try {
    writeFileSync(path, JSON.stringify(document()));
    writeFileSync(reviewPlanPathFor(path), JSON.stringify(plan()));
    writeFileSync(
      refusalPathFor(path),
      JSON.stringify({
        format: "solid-checker-contract-proposal-refusals",
        refusalVersion: 1,
        refusals: [{ entrypoint: "./types/*", stage: "entrypoint-census", reason: "open" }]
      })
    );
    const content = readContractContent(path, 0);
    assert.equal(content.measured, true);
    assert.equal(content.reviewPlanItems, 2);
    assert.equal(content.fullyProven, false);
    assert.equal(content.wireBytes.prettyMain, Buffer.byteLength(JSON.stringify(document())));
    assert.equal(content.wireBytes.proposalPlan, Buffer.byteLength(JSON.stringify(plan())));
    assert.equal(content.artifactCasesRefused, 1);
    assert.equal(content.artifactCaseRefusals[0].entrypoint, "./types/*");
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
