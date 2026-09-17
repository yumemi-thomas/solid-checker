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
  readProposalRefusalAudit,
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
  assert.deepEqual(summary.artifactCases, [
    {
      entrypoint: ".",
      caseIndex: 0,
      artifact: null,
      declarations: null,
      resolution: null
    }
  ]);
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
        refusals: [{ entrypoint: "./types/*", stage: "entrypoint-census", reason: "open" }],
        inapplicable: [
          {
            entrypoint: "./theme/base.css",
            conditions: [],
            stage: "artifact-case",
            class: "non-module-target",
            reason: "runtime target extension \".css\" is not an executable module"
          }
        ]
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
    // A recorded disposition is counted apart from refusals and never inflates
    // them.
    assert.equal(content.artifactCasesInapplicable, 1);
    assert.equal(content.artifactCaseInapplicabilities[0].class, "non-module-target");
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("a refusal audit remains readable when no contract document was emitted", () => {
  const root = mkdtempSync(join(tmpdir(), "solid-checker-refusal-only-"));
  const path = join(root, "solid-reactivity.json");
  try {
    writeFileSync(
      refusalPathFor(path),
      JSON.stringify({
        format: "solid-checker-contract-proposal-refusals",
        refusalVersion: 1,
        package: { name: "pkg", version: "1.0.0" },
        refusals: [
          { entrypoint: ".", conditions: [], stage: "artifact-case", reason: "first" },
          { entrypoint: "./sub", conditions: ["browser"], stage: "proposal-merge", reason: "second" }
        ]
      })
    );
    assert.deepEqual(readProposalRefusalAudit(path)?.refusals.map(item => item.stage), [
      "artifact-case",
      "proposal-merge"
    ]);
    assert.equal(readContractContent(path).measured, false);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("unresolved-callee declines are grouped by shape and by concrete spelling", () => {
  const declinedClosures = [
    // Two call sites of one export, one spelling: one *export* is blocked.
    { export: "a", kind: "unresolved-callee", shape: "parameter-rooted", spelling: "read" },
    { export: "a", kind: "unresolved-callee", shape: "parameter-rooted", spelling: "read" },
    { export: "b", kind: "unresolved-callee", shape: "parameter-rooted", spelling: "write" },
    { export: "c", kind: "unresolved-callee", shape: "undeclared-identifier", spelling: "fetch" },
    // Other kinds never enter the shape table, and a `dialect-silent` record
    // carries no shape at all.
    { export: "d", kind: "dialect-silent", package: "solid-js", callee: "createEffect" },
    { export: "e", kind: "refusing-callee-fixpoint", declaration: "/p/i.js:0:9" },
    // A record written before the shapes exist is counted under the empty
    // shape rather than dropped or assigned one.
    { export: "f", kind: "unresolved-callee" }
  ];
  const content = summarizeContract({ contract: document(), reviewPlan: plan(), declinedClosures });
  assert.equal(content.declinedClosures, 7);
  assert.deepEqual(content.declinedClosuresByKind, {
    "dialect-silent": 1,
    "refusing-callee-fixpoint": 1,
    "unresolved-callee": 5
  });
  assert.deepEqual(
    content.unresolvedCalleeShapes.map(shape => [shape.shape, shape.blockedExports, shape.records]),
    [
      ["parameter-rooted", 2, 3],
      // Tied on both counts, so the tie-break is the name: the unclassified
      // shape sorts first as the empty string, and is never merged away.
      ["", 1, 1],
      ["undeclared-identifier", 1, 1]
    ]
  );
  assert.deepEqual(content.unresolvedCalleeShapes[0].spellings, [
    { spelling: "read", blockedExports: 1, records: 2 },
    { spelling: "write", blockedExports: 1, records: 1 }
  ]);
  // A row with no declines at all names no shape, which is a different
  // measurement from a row whose declines carry no shape.
  assert.deepEqual(
    summarizeContract({ contract: document(), reviewPlan: plan() }).unresolvedCalleeShapes,
    []
  );
});
