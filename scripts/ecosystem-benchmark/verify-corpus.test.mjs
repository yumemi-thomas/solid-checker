import assert from "node:assert/strict";
import { test } from "node:test";

import {
  ROOT_CAUSE_ORDER,
  blockerClass,
  buildVerificationReport,
  classifyExports,
  notVerifiedLines,
  percentile,
  probeErrorBucket,
  rootCause,
  siblingPath,
  stats,
  undrivenBucket
} from "./verify-corpus.mjs";

// Real captured refusal lines from `contract verify` against the pinned
// corpus. They matter verbatim: the command writes no sidecar when it refuses,
// so this text is the only record of which RFC 0002 blocker was raised, and
// each line embeds an absolute contract path that pushes the distinguishing
// clause far into the string.
const CONTRACT = "/tmp/out-qd2X28/solid-reactivity.json";
const PROBE_REPORT = "/tmp/out-qd2X28/solid-reactivity.probe.json";

const NO_EVIDENCE_WRITE =
  `the probe report at ${PROBE_REPORT} records 4 passed claim(s) but no evidence write, so none of ` +
  `them reached the contract; re-run \`solid-checker contract probe ${CONTRACT} --write\``;
const PROBE_FAILED =
  "a probe failed: ./calendar:Root callbacks[0]=tracked: observed inline. The package does not " +
  "behave the way the contract says, and converting the claim to unknown would hide a generator " +
  "bug or a package change";
const INCOMPLETENESS =
  "an incompleteness finding contradicts a negative claim: .:createMemo invoked the callback passed " +
  "at parameter 0 in client (observed tracked), and the contract states no such claim. A negative " +
  "claim a probe falsified is wrong, not incomplete";
const KIND_UNOBSERVED =
  ".: the probe report records no passing kind observation for 3 export(s) in every mode they are " +
  "stated for: DevToolbar (development), mountDevToolbar (development), pushServerFunctionCall " +
  "(development). `kind` is the one claim schema v1 has no unknown sentinel for";
const CLOSURE_NOTE =
  ". carries a closure note: dist/esm/index.mjs: closure could not be fully enumerated: a dynamic " +
  "import() whose specifier is not a literal. The summaries were derived from a file set the " +
  "generator itself declines to claim it enumerated";
const STALE_BYTES =
  `the probe report at ${PROBE_REPORT} was written for contract bytes abc123 and ${CONTRACT} ` +
  "hashes to def456; re-probe these exact bytes before verifying them";

test("blockerClass names every RFC 0002 blocker the corpus actually raised", () => {
  assert.equal(blockerClass(NO_EVIDENCE_WRITE), "probe-report-includes-evidence-write");
  assert.equal(blockerClass(PROBE_FAILED), "probe-failed");
  assert.equal(blockerClass(INCOMPLETENESS), "incompleteness");
  assert.equal(blockerClass(KIND_UNOBSERVED), "kind-observed");
  assert.equal(blockerClass(CLOSURE_NOTE), "closure-note");
  assert.equal(blockerClass(STALE_BYTES), "probe-report-binds-contract");
  assert.equal(blockerClass(`no probe report at ${PROBE_REPORT}: mechanical verification`), "probe-report-present");
});

// The head length the harness stores has to be long enough to classify a line
// whose marker sits past an absolute path. This is the regression that
// produced a bucket of 58 "unclassified" refusals on the first pass.
test("blockerClass classifies an evidence-write refusal truncated mid-marker", () => {
  const truncated = NO_EVIDENCE_WRITE.slice(0, NO_EVIDENCE_WRITE.indexOf("claim(s)") + 5);
  assert.equal(blockerClass(truncated), "probe-report-includes-evidence-write");
});

test("blockerClass falls back rather than guessing", () => {
  assert.equal(blockerClass("something nobody has seen before"), "unclassified-refusal");
});

test("rootCause prefers a real cause over the evidence-write consequence", () => {
  const classes = new Set(["probe-report-includes-evidence-write", "incompleteness"]);
  assert.equal(rootCause(classes), "incompleteness");
  assert.equal(rootCause(new Set(["probe-report-includes-evidence-write"])), "probe-report-includes-evidence-write");
  assert.equal(rootCause(new Set(["probe-failed", "incompleteness"])), "probe-failed");
  assert.equal(rootCause(new Set()), "unclassified-refusal");
  // Every class the classifier can produce must be orderable, or a refusal
  // would silently fall through to the catch-all.
  for (const name of ROOT_CAUSE_ORDER) assert.equal(rootCause(new Set([name])), name);
});

test("undrivenBucket separates a missing probe form from a failed observation", () => {
  assert.equal(
    undrivenBucket(
      "reactive reads are proven from compiler facts and have no probe claim string: confirming one " +
        "at runtime means synthesizing a reactive source"
    ),
    "no probe form: reactiveReads"
  );
  assert.equal(
    undrivenBucket("owner requirements are proven from the compiler's canonical symbol identity"),
    "no probe form: ownerRequirements"
  );
  assert.equal(
    undrivenBucket("the synthesized call threw: TypeError: call is not a function"),
    "synthesized call threw"
  );
  assert.equal(
    undrivenBucket("the synthesized call completed without invoking the callback, so the claim was not exercised"),
    "synthesized call did not invoke the callback"
  );
  assert.equal(
    undrivenBucket("import of @solidjs/router threw: ReferenceError: window is not defined"),
    "entrypoint import threw"
  );
  assert.equal(
    undrivenBucket("spawnSync /usr/bin/node ETIMEDOUT"),
    "probe session hit the per-mode timeout"
  );
  assert.equal(
    undrivenBucket("the probe process exited 1: TypeError: callback is not a function"),
    "probe session failed (process died)"
  );
  assert.equal(undrivenBucket("a reason nobody has written yet"), "other");
});

test("probeErrorBucket names the missing runtime rather than calling it unknown", () => {
  assert.equal(
    probeErrorBucket(
      "solid-checker: no installed solid-js above /tmp/proj/node_modules/@solidjs/signals; probing " +
        "needs the project's own Solid release to settle a probe"
    ),
    "no installed solid-js beside the package"
  );
  assert.equal(probeErrorBucket(undefined), "other");
});

test("siblingPath replaces a trailing .json rather than appending to it", () => {
  assert.equal(siblingPath("/tmp/a/solid-reactivity.json", ".probe.json"), "/tmp/a/solid-reactivity.probe.json");
  assert.equal(siblingPath("/tmp/a/contract", ".verify.json"), "/tmp/a/contract.verify.json");
});

test("notVerifiedLines keeps only the refusal lines, stripped of their prefix", () => {
  const stderr = [
    "some unrelated warning",
    `solid-checker: not verified: ${PROBE_FAILED}`,
    `solid-checker: not verified: ${INCOMPLETENESS}`
  ].join("\n");
  const lines = notVerifiedLines(stderr);
  assert.equal(lines.length, 2);
  assert.equal(lines[0], PROBE_FAILED);
});

// A document dedups summaries into a `summaries` table and maps summary-id ->
// export NAMES, so counting off the raw document counts summary ids. Two
// exports sharing one summary is the case that catches it.
test("classifyExports counts export names and finds a nested unknown sentinel", () => {
  const document = {
    summaries: {
      "function-1": { kind: "function", callbacks: { status: "unknown" } },
      function: { kind: "function" }
    },
    entrypoints: {
      ".": { exports: { "function-1": ["debounce", "throttle"], function: ["scheduleIdle"] } }
    }
  };
  const expandContract = raw => ({
    entrypoints: Object.fromEntries(
      Object.entries(raw.entrypoints).map(([name, entry]) => [
        name,
        {
          exports: Object.fromEntries(
            Object.entries(entry.exports).flatMap(([id, names]) =>
              names.map(exportName => [exportName, raw.summaries[id]])
            )
          )
        }
      ])
    )
  });
  const result = classifyExports(document, expandContract);
  assert.deepEqual(result, { exports: 3, unknownBearing: 2, entrypoints: 1, expandError: null });
});

test("classifyExports records an unreadable document rather than a row of zeroes", () => {
  const result = classifyExports(null, () => {
    throw new Error("contract document is not normalized");
  });
  assert.equal(result.expandError, "contract document is not normalized");
  assert.equal(result.exports, 0);
});

test("percentile and stats report raw milliseconds, not rounded rates", () => {
  assert.equal(percentile([], 0.5), null);
  assert.equal(percentile([5, 1, 3], 0.5), 3);
  assert.deepEqual(stats([]), { count: 0, medianMs: null, p90Ms: null, maxMs: null, meanMs: null });
  const value = stats([10, 20, 30]);
  assert.equal(value.count, 3);
  assert.equal(value.maxMs, 30);
  assert.equal(value.meanMs, 20);
});

const MANIFEST = { generatedAt: "2026-08-22T07:44:17.857Z", rows: [{ probes: [{}, {}] }] };
const CHECKER = {
  nativeBin: { path: "/tmp/native", sha256: "a".repeat(64), size: 1, mtime: "2026-08-22T00:00:00.000Z" },
  typeFactsBin: { path: "/tmp/tf", sha256: "b".repeat(64), size: 1, mtime: "2026-08-22T00:00:00.000Z" }
};
const BUDGETS = { probeWallBudgetMs: 120000 };

function record(overrides) {
  return {
    probeId: "p@1|solid1|only",
    package: "p",
    version: "1",
    family: "solid-primitives",
    solidTarget: "solid1",
    totalMs: 100,
    startedAt: "2026-08-22T23:40:00.000Z",
    finishedAt: "2026-08-22T23:40:01.000Z",
    ...overrides
  };
}

// The rule this measurement exists under: a timeout is its own outcome and is
// counted as neither verified nor refused. Folding it either way is the one
// wrong answer the report could give.
test("buildVerificationReport counts a probe timeout as neither verified nor refused", () => {
  const report = buildVerificationReport({
    records: [
      record({ probeId: "a", outcome: "probe-timeout", generated: { exports: 4, unknownBearing: 0 } }),
      record({
        probeId: "b",
        outcome: "verified",
        generated: { exports: 3, unknownBearing: 1 },
        final: { exports: 3, unknownBearing: 2 },
        verify: { summary: { conversions: 1, probedRows: 0, droppedInferredMarkers: 2 }, conversions: [] }
      })
    ],
    manifest: MANIFEST,
    budgets: BUDGETS,
    checker: CHECKER
  });
  assert.equal(report.overall.rows, 2);
  assert.equal(report.overall.verified, 1);
  assert.equal(report.overall.refused, 0);
  assert.equal(report.preContractFailures.timeouts.length, 1);
  assert.equal(report.overall.outcomes["probe-timeout"], 1);
  // A timed-out row still generated a contract, and every export in it is
  // uncertified. It belongs to the composite's third state -- not to the
  // verified one, and not to nothing.
  assert.equal(report.overall.exports.certifiedInVerified, 1);
  assert.equal(report.overall.exports.unknownInVerified, 2);
  assert.equal(report.overall.exports.inUnverifiedContract, 4);
});

test("buildVerificationReport attributes a refusal to one root cause and keeps every class", () => {
  const report = buildVerificationReport({
    records: [
      record({
        probeId: "c",
        outcome: "refused",
        generated: { exports: 5, unknownBearing: 0 },
        final: { exports: 5, unknownBearing: 0 },
        blockerCount: 2,
        blockerHeads: [NO_EVIDENCE_WRITE, INCOMPLETENESS],
        probe: { summary: { claims: 6, driven: 4, passed: 3, failed: 0, undriven: 2, incompleteness: 1 } }
      })
    ],
    manifest: MANIFEST,
    budgets: BUDGETS,
    checker: CHECKER
  });
  assert.equal(report.overall.refused, 1);
  assert.equal(report.overall.rootCauses.incompleteness, 1);
  assert.equal(report.overall.blockerRows["probe-report-includes-evidence-write"], 1);
  assert.equal(report.refusals[0].rootCause, "incompleteness");
  assert.equal(report.overall.exports.inUnverifiedContract, 5);
  assert.equal(report.overall.claims.driven, 4);
});
