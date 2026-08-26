import assert from "node:assert/strict";
import { test } from "vitest";

import {
  EXECUTION_UNATTRIBUTABLE,
  OUTCOME_REASON,
  PROBE_MODES,
  UNDRIVABLE
} from "../../packages/cli/scripts/contract-probe-driver.mjs";
// The verifier's own blocker taxonomy, imported rather than restated: a kind it
// can raise and this classifier cannot name lands in `unclassified-refusal`,
// which is the one number amendment A9's stage 2 gate reads.
import { BLOCKERS } from "../../packages/cli/scripts/contract-verification.mjs";
import {
  ROOT_CAUSE_ORDER,
  blockerClass,
  buildVerificationReport,
  createConcurrencyLeasePool,
  classifyExports,
  defaultCorpusConcurrency,
  kindGapsFor,
  innerConcurrencyFor,
  notVerifiedLines,
  peerSpecsFor,
  percentile,
  probeConcurrencyForClaims,
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

const ALL_MODE_NAMES = PROBE_MODES.map(mode => mode.name);

test("innerConcurrencyFor divides the host budget across concurrent rows", () => {
  assert.equal(innerConcurrencyFor({ available: 14, rows: 6, cap: 8 }), 2);
  assert.equal(innerConcurrencyFor({ available: 4, rows: 6, cap: 8 }), 1);
  assert.equal(innerConcurrencyFor({ available: 64, rows: 6, cap: 4 }), 4);
  assert.equal(innerConcurrencyFor({ available: 8, rows: 0, cap: 4 }), 4);
});

test("defaultCorpusConcurrency leaves enough host budget for wide-row inner pools", () => {
  assert.equal(defaultCorpusConcurrency(14), 3);
  assert.equal(defaultCorpusConcurrency(2), 2);
  assert.equal(defaultCorpusConcurrency(1), 1);
});

test("wide claim plans request eight lanes while ordinary rows retain four", () => {
  assert.equal(probeConcurrencyForClaims(878), 8);
  assert.equal(probeConcurrencyForClaims(567), 8);
  assert.equal(probeConcurrencyForClaims(140), 4);
  assert.equal(probeConcurrencyForClaims(null), 4);
});

test("the phase lease never oversubscribes and wakes a queued wide row after release", async () => {
  const pool = createConcurrencyLeasePool(12);
  const first = await pool.acquire(6);
  const second = await pool.acquire(6);
  let wideGranted = false;
  const widePromise = pool.acquire(8).then(lease => {
    wideGranted = true;
    return lease;
  });
  await Promise.resolve();
  assert.equal(wideGranted, false);
  first.release();
  await Promise.resolve();
  assert.equal(wideGranted, false, "the request waits for its complete bounded lane set");
  second.release();
  const wide = await widePromise;
  assert.equal(wide.lanes, 8);
  assert.equal(pool.inUse, 8);
  wide.release();
  assert.equal(pool.inUse, 0);
});

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
// One word apart from `CLOSURE_NOTE` and a different claim: there the file set
// is not established, here it is and the runtime is what nothing bounds. Kept
// verbatim as `collectBlockers` writes it -- the first version of this rule
// classified every row whose only blocker was this one as an unclassified
// refusal, because "carries an attested closure note" does not contain "carries
// a closure note".
const ATTESTED_CLOSURE_NOTE =
  ". carries an attested closure note: index.js: the module record is attested -- it names every " +
  "file the analyzing program opened under this package -- and complete except for what a dynamic " +
  "import() whose specifier is not statically bounded to a finite set of string literals may load " +
  "at runtime, which no module graph can " +
  "enumerate. The record names every module the analysis read";

test("blockerClass names every RFC 0002 blocker the corpus actually raised", () => {
  assert.equal(blockerClass(NO_EVIDENCE_WRITE), "probe-report-includes-evidence-write");
  assert.equal(blockerClass(PROBE_FAILED), "probe-failed");
  assert.equal(blockerClass(INCOMPLETENESS), "incompleteness");
  assert.equal(blockerClass(KIND_UNOBSERVED), "kind-observed");
  assert.equal(blockerClass(CLOSURE_NOTE), "closure-note");
  // Its own class, not the closure-note bucket: merging them would make the
  // effect of attestation on this corpus unmeasurable, which is the whole reason
  // the generator emits the two on separate fields.
  assert.equal(blockerClass(ATTESTED_CLOSURE_NOTE), "attested-closure-note");
  assert.equal(blockerClass(STALE_BYTES), "probe-report-binds-contract");
  assert.equal(blockerClass(`no probe report at ${PROBE_REPORT}: mechanical verification`), "probe-report-present");
  // Amendment A9's floor: a document that would certify nothing with no `kind`
  // refusal behind it. Named rather than left to the catch-all, so that if the
  // shape ever appears the measurement says what it is.
  assert.equal(
    blockerClass(
      "no entrypoint certifies anything: the contract emits no entrypoint at all, so the promoted " +
        "document would certify nothing and the loader would reject it."
    ),
    "certifies-nothing"
  );
  // And the document-level kind line keeps its class with the phrase leading, so
  // the 260-character head cannot lose it behind a long entrypoint name.
  assert.equal(
    blockerClass(
      "no passing kind observation for any entrypoint that certifies anything: of 2 emitted " +
        "entrypoint(s), 2 are refused for an unobserved `kind` claim"
    ),
    "kind-observed"
  );
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

// The hazard `collectBlockers` documents ten lines above the code that raises
// them, and which nothing asserted until a new blocker kind slipped through: a
// refusal the verifier can raise whose class this harness cannot name is not a
// measurement, it is an unclassified row -- and `unclassified-refusal` is the
// number amendment A9's stage 2 gate reads. Two halves, because either alone
// passes while the pair is broken: every kind the verifier declares must be
// orderable as a root cause, and the classifier must actually produce it from
// the sentence.
test("every blocker kind the verifier declares is one this harness can name", () => {
  for (const kind of BLOCKERS) {
    assert.equal(
      ROOT_CAUSE_ORDER.includes(kind),
      true,
      `${kind} is raised by contract verify and has no place in ROOT_CAUSE_ORDER`
    );
    assert.equal(rootCause(new Set([kind])), kind);
  }
});

test("the closure blockers classify apart, in both directions", () => {
  const classes = new Set([blockerClass(CLOSURE_NOTE), blockerClass(ATTESTED_CLOSURE_NOTE)]);
  assert.deepEqual([...classes].sort(), ["attested-closure-note", "closure-note"]);
  // A row carrying both is a row whose record is not established at all, and
  // that is the cause a reader has to resolve first.
  assert.equal(rootCause(classes), "closure-note");
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
    undrivenBucket("the completed call did not invoke the named parameter member, so the claim was not exercised"),
    "parameter member was not invoked"
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

// The reasons a probe *result* carries, evaluated with a synthetic result so the
// package-specific detail each one interpolates is present exactly as a real one
// would be. `session-failed` forwards the session layer's own text rather than a
// string the driver owns, so it is asserted against those shapes below instead.
const RESULT = {
  export: "createStore",
  specifier: "@solid-primitives/storage",
  error: "TypeError: fn is not a function",
  outcome: "export-missing"
};

// Verbatim from packages/cli/scripts/probe-contract.mjs: `spawnSession` builds
// the first four (a spawn error's own message, a signal, a non-zero exit, an
// unparseable report) and `runSessionWithRestarts` the last two. They matter
// verbatim because they are the strings the driver forwards for
// `session-failed`, and the harness has no other handle on them.
const SESSION_FAILURES = [
  "spawnSync /opt/homebrew/bin/node ETIMEDOUT",
  "the probe process was killed by SIGTERM (timeout 20000ms)",
  "the probe process exited 1: TypeError: callback is not a function",
  "the probe process wrote no readable report: Unexpected end of JSON input",
  "the probe process stopped before reaching this claim",
  "the probe process was aborted by package code running outside a probe: Error: boom"
];

test("every reason the probe pipeline can emit has a bucket, not just the ones seen so far", () => {
  // Totality over the three tables the driver owns plus the session layer's
  // shapes and the two fallbacks `settleClaims` uses. `other` is the harness's
  // catch-all, and an unclassified bucket of 834 claims is exactly what left
  // RFC 0002 amendment A9's stage 2 undecidable: the split between "the probe
  // observed the export is absent" and "the session died" was inside it.
  for (const [name, reason] of Object.entries(UNDRIVABLE)) {
    assert.notEqual(undrivenBucket(reason), "other", `UNDRIVABLE.${name}`);
  }
  for (const [name, reason] of Object.entries(OUTCOME_REASON)) {
    if (name === "session-failed") continue;
    assert.notEqual(undrivenBucket(reason(RESULT)), "other", `OUTCOME_REASON.${name}`);
  }
  for (const reason of SESSION_FAILURES) {
    assert.notEqual(undrivenBucket(reason), "other", reason);
  }
  for (const reason of ["no probe form", "no mode was attempted", "(no reason recorded)"]) {
    assert.notEqual(undrivenBucket(reason), "other", reason);
  }
});

test("a session death that quotes a bundler export error is still a session death", () => {
  // The laundering this ordering exists to prevent. `probe-contract.mjs` builds
  // a session-failure reason as `${detail}: ${child.stderr}`, and
  // `'x' is not exported by y` is the canonical bundler message a dying package
  // prints -- so a substring rule above the session rules read a crash as the one
  // outcome amendment A9 stage 2 may narrow away. The driver's own
  // `export-missing` reason always ends `" in this mode"`, so the anchored rule
  // is exact and the ordering is the second guard.
  assert.equal(
    undrivenBucket(
      "the probe process exited 1: SyntaxError: 'createSignal' is not exported by " +
        "node_modules/solid-js/dist/server.js, imported by dist/index.js"
    ),
    "probe session failed (process died)"
  );
  assert.equal(
    undrivenBucket(
      "the probe process was killed by SIGTERM (timeout 20000ms): 'x' is not exported by y"
    ),
    "probe session failed (process died)"
  );
  assert.equal(
    undrivenBucket("spawnSync node ENOMEM: 'x' is not exported by y"),
    "probe session could not be spawned"
  );
  assert.equal(
    undrivenBucket("the probe process wrote no readable report: 'x' is not exported by y"),
    "probe session wrote no report"
  );
  // The control: an import throw quoting the same text keeps its own name.
  assert.equal(
    undrivenBucket("import of pkg threw: Error: 'x' is not exported by y"),
    "entrypoint import threw"
  );
  // And the real thing still buckets as itself.
  assert.equal(
    undrivenBucket("createStore is not exported by @solid-primitives/storage in this mode"),
    "export-missing in this mode"
  );
});

test("an observation of absence is bucketed apart from every gap", () => {
  // The distinction the next revision of the `kind` rule turns on, so it is
  // pinned rather than left to the reader of a distribution: `export-missing`
  // means the namespace loaded and the binding was not in it, which is an
  // observation that the export does not exist in that artifact. Everything
  // else here is a gap.
  assert.equal(
    undrivenBucket(OUTCOME_REASON["export-missing"](RESULT)),
    "export-missing in this mode"
  );
  assert.equal(
    undrivenBucket(OUTCOME_REASON["import-failed"](RESULT)),
    "entrypoint import threw"
  );
  assert.equal(
    undrivenBucket("the probe process was aborted by package code running outside a probe: Error: x"),
    "probe session aborted by package code"
  );
  assert.equal(undrivenBucket("no mode was attempted"), "no mode was attempted");
  // A reworded session failure still lands in a name rather than in `other`.
  assert.equal(
    undrivenBucket("the probe process gave up in some way nobody has written yet"),
    "probe session failed (other)"
  );
  assert.equal(undrivenBucket("spawnSync /usr/bin/node ENOENT"), "probe session could not be spawned");
});

// ---------------------------------------------------------------------------
// Why a `kind` observation is missing
// ---------------------------------------------------------------------------

test("kindGapsFor splits an observation of absence from a gap and from a contradiction", () => {
  const gaps = kindGapsFor([
    // Observed everywhere: not a gap at all.
    {
      export: "createSignal",
      claim: "kind=function",
      status: "passed",
      modes: { attempted: ["client", "server"], passed: ["client", "server"] }
    },
    // The shape amendment A9 stage 2 is about: passing in the browser modes,
    // and the export simply does not exist in the server artifact.
    {
      export: "useLocation",
      claim: "kind=function",
      status: "undriven",
      modes: { attempted: ["client", "server"], passed: ["client"] },
      observations: [
        { mode: "client", status: "passed" },
        {
          mode: "server",
          status: "undriven",
          reason: "useLocation is not exported by @solidjs/router in this mode"
        }
      ]
    },
    // A gap: nothing was observed, so nothing may be narrowed away.
    {
      export: "Router",
      claim: "kind=function",
      status: "undriven",
      modes: { attempted: ["client", "server"], passed: [] },
      observations: [
        {
          mode: "client",
          status: "undriven",
          reason: "import of @solidjs/router threw: ReferenceError: window is not defined"
        },
        { mode: "server", status: "undriven", reason: "the probe process exited 1" }
      ]
    },
    // A contradiction, which must never be counted as a gap: the package
    // answered the claim differently, and that is a failure to fix rather than
    // a mode to exclude.
    {
      export: "ReactiveMap",
      claim: "kind=value",
      status: "failed",
      modes: { attempted: ["client"], passed: [] },
      observations: [{ mode: "client", status: "failed", reason: "runtime kind is function" }]
    },
    // Not a `kind` claim, so it is none of this function's business.
    {
      export: "createSignal",
      claim: "callbacks[0]=tracked",
      status: "undriven",
      modes: { attempted: ["client"], passed: [] },
      observations: [{ mode: "client", status: "undriven", reason: "anything at all" }]
    }
  ]);
  // A contradiction is in neither `claims` nor `modes`: amendment A9 says the
  // two must never share a number, and the markdown headings above these
  // figures say "unobserved". Sharing them and separating only `reasons` was
  // that failure with a label on it -- the corpus carries 53 contradicted `kind`
  // claims across 20 rows, every one of which would have been counted as a gap.
  assert.equal(gaps.claims, 2);
  assert.deepEqual(gaps.modes, { client: 1, server: 2 });
  assert.deepEqual(gaps.reasons, {
    "export-missing in this mode": 1,
    "entrypoint import threw": 1,
    "probe session failed (process died)": 1
  });
  assert.equal(gaps.contradictions.claims, 1);
  assert.deepEqual(gaps.contradictions.modes, { client: 1 });
  assert.deepEqual(gaps.contradictions.reasons, { "observed and did not pass (failed)": 1 });
});

test("a claim gapped in one mode and contradicted in another is counted in both, once each", () => {
  const gaps = kindGapsFor([
    {
      export: "ReactiveMap",
      claim: "kind=value",
      status: "failed",
      modes: { attempted: ["client", "server"], passed: [] },
      observations: [
        { mode: "client", status: "failed", reason: "runtime kind is function" },
        { mode: "server", status: "undriven", reason: "the probe process exited 1" }
      ]
    }
  ]);
  assert.equal(gaps.claims, 1);
  assert.deepEqual(gaps.modes, { server: 1 });
  assert.equal(gaps.contradictions.claims, 1);
  assert.deepEqual(gaps.contradictions.modes, { client: 1 });
});

test("an attempted mode with no observation at all is its own gap", () => {
  const gaps = kindGapsFor([
    {
      export: "createSignal",
      claim: "kind=function",
      status: "undriven",
      modes: { attempted: ["client"], passed: [] }
    }
  ]);
  assert.deepEqual(gaps.reasons, { "no observation recorded for the mode": 1 });
});

test("a mode the run never attempted is a labelled gap, not an absence from the table", () => {
  // Two of the four non-observing outcomes A9's stage-0 table enumerates are not
  // per-mode observations at all, so iterating `modes.attempted` could not see
  // them. A `--modes client` run attempts one mode and the verifier still
  // refuses the entrypoint for the other three; the run's own mode list is what
  // makes that visible.
  const claim = {
    export: "createSignal",
    claim: "kind=function",
    status: "passed",
    modes: { attempted: ["client"], passed: ["client"] }
  };
  assert.deepEqual(kindGapsFor([claim], { modes: ["client"] }), {
    claims: 1,
    modes: { server: 1, development: 1, production: 1 },
    reasons: { "the run never attempted this mode": 3 },
    contradictions: { claims: 0, modes: {}, reasons: {} }
  });
  // A run that drove every mode has none of them, which is why the corpus's
  // own figure should be zero and a non-zero one means the run was narrowed.
  assert.equal(kindGapsFor([claim], { modes: ALL_MODE_NAMES }).claims, 0);
});

test("a mode where no unambiguous summary resolves is a gap the plan records elsewhere", () => {
  // `buildProbePlan` creates no `kind=` claim for such a mode at all -- it
  // records a family-(C) `summary` claim naming the mode -- so a rule that read
  // only `kind=` claims left the verifier's "(no unambiguous summary resolves
  // there)" refusal invisible to the measurement that gates it.
  const gaps = kindGapsFor([
    {
      entrypoint: ".",
      export: "createAsync",
      claim: "summary",
      family: "C",
      status: "undriven",
      reason: "no unambiguous summary in server",
      modes: { attempted: [], passed: [] }
    }
  ]);
  assert.equal(gaps.claims, 1);
  assert.deepEqual(gaps.modes, { server: 1 });
  assert.deepEqual(gaps.reasons, {
    "no unambiguous summary resolves in the mode (no kind claim exists)": 1
  });
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

test("the report carries the kind-gap breakdown and the entrypoints verification refused", () => {
  const report = buildVerificationReport({
    records: [
      record({
        probeId: "v",
        outcome: "verified",
        generated: { exports: 2, unknownBearing: 0 },
        final: { exports: 1, unknownBearing: 0 },
        verify: {
          summary: { conversions: 0, probedRows: 1, refusedEntrypoints: 1 },
          conversions: [],
          refusedEntrypoints: [
            { entrypoint: "./server", blocker: "./server: ... no passing kind observation", exports: 1 }
          ]
        },
        probe: {
          summary: { claims: 4, driven: 2, passed: 2, failed: 0, undriven: 2, incompleteness: 0 },
          kindGaps: {
            claims: 1,
            modes: { server: 1 },
            reasons: { "export-missing in this mode": 1 }
          }
        }
      }),
      record({
        probeId: "r",
        outcome: "refused",
        generated: { exports: 3, unknownBearing: 0 },
        final: { exports: 3, unknownBearing: 0 },
        blockerCount: 1,
        blockerHeads: [KIND_UNOBSERVED],
        probe: {
          summary: { claims: 3, driven: 0, passed: 0, failed: 0, undriven: 3, incompleteness: 0 },
          kindGaps: {
            claims: 3,
            modes: { client: 3, server: 3 },
            reasons: { "entrypoint import threw": 6 },
            // The same row also carries a contradicted claim. It must land in
            // the contradiction totals and in none of the gap ones.
            contradictions: {
              claims: 1,
              modes: { client: 1 },
              reasons: { "observed and did not pass (failed)": 1 }
            }
          }
        }
      })
    ],
    manifest: MANIFEST,
    budgets: BUDGETS,
    checker: CHECKER
  });
  assert.equal(report.overall.kindGaps.rows, 2);
  assert.equal(report.overall.kindGaps.claims, 4);
  assert.deepEqual(report.overall.kindGaps.reasons, {
    "export-missing in this mode": 1,
    "entrypoint import threw": 6
  });
  assert.equal(report.overall.kindGaps.contradictions.rows, 1);
  assert.equal(report.overall.kindGaps.contradictions.claims, 1);
  assert.deepEqual(report.overall.kindGaps.contradictions.modes, { client: 1 });
  assert.equal(report.overall.verificationRefusedEntrypoints, 1);
  assert.equal(report.overall.rowsWithAVerificationRefusedEntrypoint, 1);
  // A refused row's gaps are carried too: a row root-caused elsewhere can still
  // have one, and those are the rows that must stay refused.
  assert.equal(report.refusals[0].kindGaps.claims, 3);
  assert.deepEqual(report.verified[0].refusedEntrypoints, ["./server"]);
  const markdown = renderVerificationMarkdown(report);
  assert.match(markdown, /### Why a `kind` observation is missing/);
  assert.match(markdown, /\| export-missing in this mode \| 1 \|/);
  assert.match(markdown, /Entrypoints verification refused inside a promoted document \| 1 \|/);
  assert.match(markdown, /cost made visible, not a regression/);
  // The contradictions render as their own section, and no gap heading counts
  // them: the two numbers can never be read as one.
  assert.match(markdown, /### `kind` claims the probe contradicted/);
  assert.match(markdown, /- `kind` claims contradicted in at least one mode: 1/);
  assert.match(markdown, /- `kind` obligations with at least one gapped stated mode: 4/);
  assert.equal(/observed and did not pass[\s\S]*?Why the mode produced no passing/.test(markdown), false);
  // The per-row half: which refusals are absences, which are gaps, and which
  // carry a contradiction.
  assert.match(markdown, /\| `r` \| .* \| entrypoint import threw x6, \*\*contradicted\*\* x1 \|/);
  // And the named survivor, so "verified" is never read without "minus this".
  assert.match(markdown, /\| `v` \| 1 \| 0 \| 0 \| 1 \| `\.\/server` \|/);
});

test("the composite keeps a verification-refused export in its denominator", () => {
  // The flattering direction the re-measurement plan forbids. Both verified
  // states are counted off `record.final` -- the *promoted* document -- so the
  // exports that left with a refused entrypoint were in none of the composite's
  // states, and stage 1 raised the certified share for a reason with no
  // certification behind it. Here: a 21-export draft, 2 of 3 entrypoints
  // refused, 7 exports promoted of which 5 certify.
  const report = buildVerificationReport({
    records: [
      record({
        probeId: "v",
        outcome: "verified",
        generated: { exports: 21, unknownBearing: 0 },
        final: { exports: 7, unknownBearing: 2 },
        verify: {
          summary: { conversions: 2, probedRows: 0, refusedEntrypoints: 2 },
          conversions: [],
          refusedEntrypoints: [
            { entrypoint: "./server", blocker: "./server: ...", exports: 8 },
            { entrypoint: "./web", blocker: "./web: ...", exports: 6 }
          ]
        },
        probe: { summary: { claims: 21, driven: 7, passed: 5, failed: 0, undriven: 14, incompleteness: 0 } }
      })
    ],
    manifest: MANIFEST,
    budgets: BUDGETS,
    checker: CHECKER
  });
  assert.equal(report.overall.exports.certifiedInVerified, 5);
  assert.equal(report.overall.exports.unknownInVerified, 2);
  assert.equal(report.overall.exports.refusedInVerified, 14);
  assert.equal(report.overall.exports.inUnverifiedContract, 0);
  const markdown = renderVerificationMarkdown(report);
  // 5 of 21, not 5 of 7.
  assert.match(markdown, /\| \(a\) certified by a verified contract \| 5\/21 \(23\.81%\) \|/);
  assert.match(markdown, /\| \(b\) honest unknown inside a verified contract \| 2\/21/);
  assert.match(
    markdown,
    /\| \(c\) dropped from a verified contract with its refused entrypoint \| 14\/21/
  );
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
