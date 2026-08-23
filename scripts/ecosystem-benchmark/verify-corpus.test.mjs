import assert from "node:assert/strict";
import { test } from "node:test";

import { EXECUTION_UNATTRIBUTABLE } from "../../packages/cli/scripts/contract-probe-driver.mjs";
import {
  ROOT_CAUSE_ORDER,
  blockerClass,
  buildVerificationReport,
  classifyExports,
  notVerifiedLines,
  peerSpecsFor,
  percentile,
  probeBudgetFor,
  probeErrorBucket,
  probeFailureShape,
  renderVerificationMarkdown,
  rootCause,
  runtimeSpecsFor,
  siblingPath,
  stats,
  undrivenBucket
} from "./verify-corpus.mjs";

// Real captured refusal lines from `contract verify` against the pinned
// corpus. They matter verbatim: they are what the refusal sidecar's
// `blockers.raised` carries and what an older journal captured from stderr,
// and each line embeds an absolute contract path that pushes the
// distinguishing clause far into the string.
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

test("every reason the probe driver can give for an unattributable observation has a bucket", () => {
  // The distribution this feeds is how a corpus measurement is read, and a
  // reason the buckets do not know lands in `other` together with everything
  // else unrecognized -- which is worst exactly when a new withdrawal class is
  // the largest one in the run. Asserting over the driver's own table rather
  // than over a copied list is what makes the next reason string fail here
  // instead of quietly widening `other`.
  for (const [name, reason] of Object.entries(EXECUTION_UNATTRIBUTABLE)) {
    assert.notEqual(undrivenBucket(reason), "other", name);
  }
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

// ---------------------------------------------------------------------------
// The install environment
// ---------------------------------------------------------------------------

const RELEASES = {
  solidReleases: {
    "@solidjs/web": { v2: ["2.0.0-rc.0", "2.0.0-rc.1"] }
  }
};

test("a Solid 2 row pinning only solid-js gets the @solidjs/web half of the same runtime", () => {
  const completed = runtimeSpecsFor({
    probe: { solid: { "solid-js": "2.0.0-rc.1" } },
    manifest: RELEASES
  });
  assert.deepEqual(completed.pinned, { "solid-js": "2.0.0-rc.1", "@solidjs/web": "2.0.0-rc.1" });
  assert.deepEqual(completed.added, ["@solidjs/web"]);
});

test("a Solid 1 row is never given a Solid 2 companion", () => {
  const untouched = runtimeSpecsFor({ probe: { solid: { "solid-js": "1.9.14" } }, manifest: RELEASES });
  assert.deepEqual(untouched.pinned, { "solid-js": "1.9.14" });
  assert.deepEqual(untouched.added, []);
});

test("a version the corpus never audited is not substituted in to make a row work", () => {
  const unaudited = runtimeSpecsFor({
    probe: { solid: { "solid-js": "2.0.0-beta.19" } },
    manifest: RELEASES
  });
  assert.deepEqual(unaudited.pinned, { "solid-js": "2.0.0-beta.19" });
  assert.deepEqual(unaudited.added, []);
});

test("a row that already pins both is left exactly as the manifest wrote it", () => {
  const both = { "solid-js": "2.0.0-rc.0", "@solidjs/web": "2.0.0-rc.0" };
  const result = runtimeSpecsFor({ probe: { solid: both }, manifest: RELEASES });
  assert.deepEqual(result.pinned, both);
  assert.deepEqual(result.added, []);
});

test("peers come from the installed artifact, and a runtime peer is skipped with a reason", () => {
  const { specs, skipped } = peerSpecsFor({
    installedManifest: {
      peerDependencies: {
        "solid-js": ">=1.9.7",
        "@solidjs/web": "^2.0.0-rc.0",
        vinxi: "^0.5.7",
        typescript: "^5.0.0"
      },
      peerDependenciesMeta: { typescript: { optional: true } }
    },
    pinned: { "solid-js": "1.9.14" }
  });
  assert.deepEqual(specs, [{ package: "vinxi", range: "^0.5.7" }]);
  assert.deepEqual(skipped, [
    { package: "@solidjs/web", reason: "a Solid runtime package the row does not pin" },
    { package: "solid-js", reason: "already pinned by the manifest row" },
    { package: "typescript", reason: "declared optional by the package" }
  ]);
});

test("a package declaring no peers asks for no second install", () => {
  assert.deepEqual(peerSpecsFor({ installedManifest: {}, pinned: {} }), { specs: [], skipped: [] });
});

// ---------------------------------------------------------------------------
// The probe budget
// ---------------------------------------------------------------------------

test("the probe budget scales with the planned claim count and is capped", () => {
  const budget = { base: 60_000, perClaim: 150, cap: 420_000 };
  // A one-export primitive gets the base and nothing more.
  assert.equal(probeBudgetFor({ claims: 8, ...budget }), 61_200);
  // A wide surface gets proportionally more...
  assert.equal(probeBudgetFor({ claims: 1000, ...budget }), 210_000);
  // ...until the cap, which is what keeps one package from holding a worker
  // for the length of the run.
  assert.equal(probeBudgetFor({ claims: 100_000, ...budget }), 420_000);
});

test("a row whose claim count could not be planned falls back to the base budget", () => {
  const budget = { base: 60_000, perClaim: 150, cap: 420_000 };
  assert.equal(probeBudgetFor({ claims: null, ...budget }), 60_000);
  assert.equal(probeBudgetFor({ claims: 0, ...budget }), 60_000);
});

// ---------------------------------------------------------------------------
// Probe failures
// ---------------------------------------------------------------------------

test("a failure is reduced to the claim, what was claimed, and what was observed", () => {
  assert.equal(
    probeFailureShape({ claim: "callbacks[0]=tracked", observed: "deferred" }),
    "callbacks[n]: claimed tracked, observed deferred"
  );
  assert.equal(
    probeFailureShape({ claim: "callbacks[2]=tracked", observed: "inline" }),
    "callbacks[n]: claimed tracked, observed inline"
  );
  assert.equal(
    probeFailureShape({ claim: "returns=accessor", observed: "object" }),
    "returns: claimed accessor, observed object"
  );
});

test("a failure with no recorded observation recovers one from the reason, or says so", () => {
  assert.equal(
    probeFailureShape({ claim: "kind=function", reason: "runtime kind is value" }),
    "kind: claimed function, observed value"
  );
  assert.equal(
    probeFailureShape({ claim: "callbacks[0]=inline" }),
    "callbacks[n]: claimed inline, observed not observed"
  );
});

test("the report groups probe failures by shape and names every one of them", () => {
  const failures = [
    { entrypoint: ".", export: "a", claim: "callbacks[0]=tracked", observed: "deferred", modes: ["client"] },
    { entrypoint: ".", export: "b", claim: "callbacks[1]=tracked", observed: "deferred", modes: ["server"] },
    { entrypoint: ".", export: "c", claim: "returns=accessor", observed: "object", modes: ["client"] }
  ];
  const report = buildVerificationReport({
    records: [
      record({
        probeId: "z",
        outcome: "refused",
        generated: { exports: 3, unknownBearing: 0 },
        final: { exports: 3, unknownBearing: 0 },
        blockerCount: 1,
        blockerHeads: [PROBE_FAILED],
        probe: {
          summary: { claims: 3, driven: 3, passed: 0, failed: 3, undriven: 0, incompleteness: 0 },
          failures
        }
      })
    ],
    manifest: MANIFEST,
    budgets: BUDGETS,
    checker: CHECKER
  });
  assert.equal(report.probeFailures.rows.length, 3);
  assert.equal(report.probeFailures.shapes["callbacks[n]: claimed tracked, observed deferred"], 2);
  assert.equal(report.probeFailures.shapes["returns: claimed accessor, observed object"], 1);
  const markdown = renderVerificationMarkdown(report);
  assert.match(markdown, /## Probe failures: claims the package answered differently/);
  assert.match(markdown, /callbacks\[n\]: claimed tracked, observed deferred/);
  // The individual rows carry the modes, because "deferred in server only" and
  // "deferred everywhere" are different findings.
  assert.match(markdown, /\| `z` \| `\.:a` \| `callbacks\[0\]=tracked` \| deferred \| client \|/);
});

// ---------------------------------------------------------------------------
// The environment and session records
// ---------------------------------------------------------------------------

test("the report says which globals were faked, in which modes, and on how many rows", () => {
  const environment = {
    shimmedAnyMode: true,
    modes: {
      client: { kind: "browser-globals", shimmed: ["document", "window"], present: ["navigator"] },
      development: { kind: "browser-globals", shimmed: ["window"], present: [] },
      server: { kind: "none", shimmed: [], present: [] }
    }
  };
  const report = buildVerificationReport({
    records: [
      record({
        probeId: "s",
        outcome: "verified",
        generated: { exports: 1, unknownBearing: 0 },
        final: { exports: 1, unknownBearing: 0 },
        verify: { summary: { conversions: 0, probedRows: 0 }, conversions: [] },
        probe: {
          summary: { claims: 1, driven: 1, passed: 1, failed: 0, undriven: 0, incompleteness: 0 },
          environment,
          sessions: { started: 6, restarts: 2, failed: 1, byMode: {} }
        }
      })
    ],
    manifest: MANIFEST,
    budgets: BUDGETS,
    checker: CHECKER
  });
  assert.equal(report.probeEnvironment.shim.rowsShimmed, 1);
  assert.equal(report.probeEnvironment.shim.shimmedGlobals.window, 1);
  assert.equal(report.probeEnvironment.shim.shimmedGlobals.document, 1);
  assert.equal(report.probeEnvironment.shim.modesShimmed.client, 1);
  assert.equal(report.probeEnvironment.shim.modesShimmed.server, undefined);
  assert.deepEqual(report.probeEnvironment.sessions, { started: 6, restarts: 2, failed: 1 });
  const markdown = renderVerificationMarkdown(report);
  assert.match(markdown, /### The globals the probe worker faked/);
  assert.match(markdown, /weaker observation than one made in a browser/);
  assert.match(markdown, /`server` sessions are never shimmed/);
});

// ---------------------------------------------------------------------------
// No runtime
// ---------------------------------------------------------------------------

test("a row with no honest Solid runtime is its own class, not an error and not a refusal", () => {
  const report = buildVerificationReport({
    records: [
      record({
        probeId: "@solidjs/signals@2.0.0-rc.1|solid2|head",
        outcome: "no-runtime",
        generated: { exports: 20, unknownBearing: 0 },
        detail: "the manifest pins {} for this row"
      })
    ],
    manifest: MANIFEST,
    budgets: BUDGETS,
    checker: CHECKER
  });
  assert.equal(report.overall.verified, 0);
  assert.equal(report.overall.refused, 0);
  assert.equal(report.overall.outcomes["no-runtime"], 1);
  assert.equal(report.preContractFailures.noRuntime.length, 1);
  // It generated a contract, so its exports are in the composite's third state.
  assert.equal(report.overall.exports.inUnverifiedContract, 20);
  assert.match(renderVerificationMarkdown(report), /no Solid runtime the row could honestly be probed against/);
});

test("the install record reaches the report", () => {
  const report = buildVerificationReport({
    records: [
      record({
        probeId: "i",
        outcome: "verified",
        generated: { exports: 1, unknownBearing: 0 },
        final: { exports: 1, unknownBearing: 0 },
        verify: { summary: { conversions: 0, probedRows: 0 }, conversions: [] },
        install: {
          pinned: ["p@1.0.0", "solid-js@2.0.0-rc.1"],
          runtimeCompleted: ["@solidjs/web"],
          peers: ["vinxi@^0.5.7"],
          peersSkipped: [],
          peerInstall: "complete"
        }
      })
    ],
    manifest: MANIFEST,
    budgets: BUDGETS,
    checker: CHECKER
  });
  assert.equal(report.installEnvironment.runtimeCompleted, 1);
  assert.equal(report.installEnvironment.peerComplete, 1);
  assert.equal(report.installEnvironment.peersInstalled, 1);
  assert.match(renderVerificationMarkdown(report), /## The install environment/);
});
