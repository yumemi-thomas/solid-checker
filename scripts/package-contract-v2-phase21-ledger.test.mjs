import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { test } from "vitest";

import {
  buildPhase21LedgerFromBaseline,
  classifyPhase21Terminal
} from "./package-contract-v2-phase21-ledger.mjs";

test("Phase 21 terminal taxonomy preserves semantic terminal authority before artifact reason text", () => {
  const misleadingReason =
    "dependency contract for @scope/dep has no entrypoint matching \".\"; resolved target <package-root>/dist/index.js is not a file";

  assert.equal(
    classifyPhase21Terminal({ observedClass: "dependency-contract-obligation", artifactCases: [{ reason: misleadingReason }] }),
    "dependency-contract-obligation"
  );
  assert.equal(
    classifyPhase21Terminal({ observedClass: "export-kind-conflict", artifactCases: [{ reason: misleadingReason }] }),
    "export-kind-conflict"
  );
  assert.equal(
    classifyPhase21Terminal({ observedClass: "export-kind-unresolved", artifactCases: [{ reason: misleadingReason }] }),
    "export-kind-unresolved"
  );
});

test("Phase 21 terminal taxonomy names every bounded package and artifact failure", () => {
  const classify = reason => classifyPhase21Terminal({ observedClass: "unclassified", artifactCases: [{ reason }] });

  assert.equal(
    classify("resolved target <package-root>/dist/index.jsx is not a file"),
    "published-target-missing"
  );
  assert.equal(
    classify("local closure module ./types.js from <package-root>/dist/index.d.ts was not found"),
    "published-declaration-closure-missing"
  );
  assert.equal(
    classify(
      "local closure module ../node_modules/solid-js/types/reactive/signal.js from <package-root>/dist/index.d.ts was not found"
    ),
    "authenticated-dependency-layout-required"
  );
});

test("the frozen Phase 20 cohort remains the exact authority for the Phase 21 ledger", () => {
  const baselineBytes = readFileSync(
    new URL("../benchmarks/package-contract-v2/phase21/baseline-cohort.json", import.meta.url)
  );
  const reportBytes = readFileSync(new URL("../benchmarks/ecosystem/report.json", import.meta.url));
  const baseline = JSON.parse(baselineBytes.toString("utf8"));
  const report = JSON.parse(reportBytes.toString("utf8"));
  const ledger = buildPhase21LedgerFromBaseline({
    baseline,
    currentReport: report,
    currentReportSha256: createHash("sha256").update(reportBytes).digest("hex"),
    baselineSha256: createHash("sha256").update(baselineBytes).digest("hex")
  });

  assert.equal(ledger.summary.rows, 30);
  assert.equal(ledger.summary.retainedControls.upstreamMissingBytes, 5);
  assert.equal(ledger.summary.retainedControls.cjsNoEsm, 7);
  assert.equal(ledger.summary.checkerAddressableRows, 18);
  assert.equal(ledger.summary.terminalClasses.unclassified, undefined);
  // Self-invalidating pin: this is the digest the test itself computes over
  // benchmarks/ecosystem/report.json, so regenerating that report makes this
  // literal stale by construction. Re-pin it from the regenerated file
  // (`shasum -a 256 benchmarks/ecosystem/report.json`) in the same change that
  // regenerates the report, together with the disposition census below, and
  // only once `bun scripts/package-contract-v2-phase21-ledger.mjs --check`
  // passes against the rewritten ledger.
  assert.deepEqual(ledger.authority.currentReport, {
    path: "benchmarks/ecosystem/report.json",
    sha256: "727667e112c7aa56bd34c312e14fcffd20ea2ef88ab25caffcde09f771295923"
  });
  assert.equal(ledger.rows.filter(row => row.phase21Disposition == null).length, 0);
  // One row moved, `@solid-primitives/geolocation@1.5.5|solid1|only`:
  // `partial-upstream-target-missing`/`upstream-package` ->
  // `pending-phase21-checker-work`/`checker-semantic-model`.
  //
  // Cause: the `unpublished-conditional-target` inapplicable disposition
  // (docs/precision-backlog.md, 2026-08-31). The row's second artifact case
  // selected `<package-root>/src/index.ts` through the private namespaced
  // condition `@solid-primitives/source`, and that target is deliberately
  // absent from the tarball; it is now recorded inapplicable instead of
  // refused. Its `.` case already certified, so the row's proposal became
  // `complete` and it certifies verified — and `phase21Disposition`'s
  // geolocation arm recognizes only `partial` + `published-target-missing` as
  // `partial-upstream-target-missing`, so a *verified* row falls through to
  // its else branch. NOT caused by the `export-kind-conflict` marker
  // promotion in scripts/ecosystem-benchmark/lib/classify.mjs: that arm reads
  // only `after`, and a `success` outcome short-circuits
  // `effectiveClassification` before `classifyResult` is ever called.
  //
  // Known gap this pin records rather than endorses: unlike the corvu arm,
  // the geolocation arm has no `verified` case, so it labels a row that now
  // holds an authenticated ordinary receipt as pending checker work.
  assert.deepEqual(ledger.summary.dispositionStates, {
    "confirmed-upstream-declaration-defect": 1,
    "exact-refusal-authenticated-layout": 5,
    "exact-refusal-package-import-resolution": 3,
    "exact-refusal-semantic-model": 5,
    "pending-phase21-checker-work": 3,
    "retained-unsupported-runtime-model": 7,
    "retained-upstream-missing-bytes": 5,
    "verified-through-ordinary-receipt-load": 1
  });
  assert.deepEqual(ledger.summary.remainingOwners, {
    "authenticated-dependency-layout": 5,
    "checker-resolver": 3,
    "checker-semantic-model": 6,
    "checker-type-facts": 2,
    none: 1,
    "runtime-model": 7,
    "upstream-package": 6
  });

  const context = ledger.rows.find(row => row.probeId === "@solid-primitives/context@0.3.2|solid1|only");
  assert.equal(context.before.terminalClass, "authenticated-dependency-layout-required");
  assert.equal(context.before.integrityVerified, true);
  assert.deepEqual(context.before.installedVersions, {
    "@solid-primitives/context": "0.3.2",
    "solid-js": "1.9.14"
  });
  assert.deepEqual(
    context.before.artifactCases,
    baseline.rows.find(row => row.probeId === context.probeId).before.artifactCases
  );
  assert.deepEqual(context.before.dependencyPlan, null);
  assert.equal(context.before.disposition, "proposal-blocked");
  assert.deepEqual(context.phase21Disposition, {
    state: "confirmed-upstream-declaration-defect",
    remainingOwner: "upstream-package",
    evidence: "docs/package-contract-v2/phase21/context-upstream-declaration-defect.md"
  });
  assert.equal(ledger.summary.confirmedUpstreamDefects, 1);
});
