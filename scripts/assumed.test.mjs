// ADR 0950: scripts/lib/assumed.mjs, the predicate and the refusal every consumer of the checker's JSON report applies at its intake. This file
// holds the helper alone; each consumer's own test feeds its real intake a tainted report (next to the consumer's other tests). It lives here
// and not in scripts/lib/ because `scripts/*.test.mjs` is the glob `make verify` and CI run.
import assert from "node:assert/strict";
import { test } from "vitest";

import { AssumedReportError, isAssumed, refuseAssumed } from "./lib/assumed.mjs";
import { HAZARD, TAINTED, cleanReport, finding, flippedReport } from "./lib/assumed-fixtures.mjs";

test("a report with a top-level `assumed` key is tainted, whatever the key holds", () => {
  for (const value of [{ hazards: [HAZARD], flipped: [] }, { hazards: [], flipped: [] }, null, false, 0, "", [], {}]) {
    assert.equal(isAssumed({ status: "certified", findings: [], assumed: value }), true, JSON.stringify(value));
  }
  // The key alone is the taint: a what-if build that flipped nothing writes it and no premise.
  assert.equal(isAssumed(TAINTED.unflipped()), true);
});

test("a finding with a premise of kind `assumed` taints the report with no top-level key", () => {
  assert.equal(isAssumed(TAINTED.premiseOnly()), true);
  // One such finding among clean ones is enough, wherever it stands in the list or in its own premises.
  const premises = [{ kind: "SiteReachAbsentProp" }, { kind: "assumed", hazards: [HAZARD] }];
  assert.equal(isAssumed({ status: "violation", findings: [finding(), finding(), finding({ premises })] }), true);
  // The whole shape a what-if build writes when a hazard flipped.
  assert.equal(isAssumed(flippedReport()), true);
});

test("a report as a default build writes it is not tainted, nor is one that only looks alike", () => {
  assert.equal(isAssumed(cleanReport()), false);
  assert.equal(isAssumed({ status: "certified", findings: [] }), false);
  assert.equal(isAssumed({ status: "certified" }), false);
  // The report's own premises are strings (the project-wide ones); a premise of another kind, or another spelling, is not this one.
  assert.equal(isAssumed({ findings: [], premises: ["standard runtime configuration (ADRs 0266/0268)"] }), false);
  assert.equal(isAssumed({ findings: [finding({ premises: [{ kind: "SiteReachAbsentProp" }, { kind: "Assumed" }, "assumed", null] })] }), false);
  // Not the top-level key: a finding's own `assumed` field, another name, a nested report.
  assert.equal(isAssumed({ findings: [finding({ assumed: true })] }), false);
  assert.equal(isAssumed({ findings: [], assumedHazards: [HAZARD] }), false);
  assert.equal(isAssumed({ findings: [], nested: { assumed: {} } }), false);
  // Findings that are not a list, or a list with holes, are the reader's to refuse, not a taint.
  assert.equal(isAssumed({ findings: "none" }), false);
  assert.equal(isAssumed({ findings: [null, 3, {}, { premises: "assumed" }] }), false);
});

test("anything that is no report object is not tainted", () => {
  for (const value of [null, undefined, 0, 1, "assumed", true, [], [{ assumed: {} }]]) {
    assert.equal(isAssumed(value), false, JSON.stringify(value) ?? "undefined");
  }
});

test("refuseAssumed gives back the very report it was handed when it is untainted", () => {
  const report = cleanReport();
  assert.equal(refuseAssumed(report, "scripts/x.mjs"), report);
  assert.equal(refuseAssumed(null, "scripts/x.mjs"), null);
  assert.equal(refuseAssumed(undefined, "scripts/x.mjs"), undefined);
  assert.deepEqual(report, cleanReport());
});

test("refuseAssumed throws an AssumedReportError that names the consumer, the what-if assumption and the one script that reads such a report", () => {
  for (const [name, make] of Object.entries(TAINTED)) {
    assert.throws(
      () => refuseAssumed(make(), "scripts/some-gate.mjs"),
      (error) => {
        assert.ok(error instanceof AssumedReportError, name);
        assert.ok(error instanceof Error, name);
        assert.equal(error.name, "AssumedReportError");
        assert.match(error.message, /^scripts\/some-gate\.mjs refuses this checker report/, name);
        assert.match(error.message, /tainted by a what-if assumption/, name);
        assert.match(error.message, /Only scripts\/what-if\.mjs reads one\./, name);
        return true;
      },
      name,
    );
  }
});

test("the refusal says which hazards were assumed when the report names them, and still refuses when it does not", () => {
  assert.throws(() => refuseAssumed(flippedReport(), "x"), new RegExp(`\\(assumed: ${HAZARD.replaceAll(/[()*]/g, "\\$&")}\\)`));
  assert.throws(() => refuseAssumed({ findings: [], assumed: { hazards: ["RootPatch", "StandardRootHanded"], flipped: [] } }, "x"), /\(assumed: RootPatch, StandardRootHanded\)/);
  for (const assumed of [null, "RootPatch", {}, { hazards: "RootPatch" }]) {
    assert.throws(() => refuseAssumed({ findings: [], assumed }, "x"), (error) => error instanceof AssumedReportError && !/\(assumed:/.test(error.message));
  }
});
