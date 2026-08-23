// `solid-checker contract probe` -- RFC 0002 Stage 1.
//
// Everything the driver decides is tested hermetically with an injected fake
// runtime: no install, no package code, no native binary. The one test that
// drives a real release is gated on an npm install succeeding and skips cleanly
// when it cannot.

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import test from "node:test";

import { expandContract } from "../packages/cli/scripts/contract-document.mjs";
import {
  collectReviewItems,
  renderReviewPlanDocument
} from "../packages/cli/scripts/contract-review-plan.mjs";
import {
  ARGUMENT_SYNTHESIS,
  PROBE_MODES,
  applyProbeEvidence,
  attemptedModes,
  buildProbePlan,
  classifyExecution,
  interpretSession,
  settleClaims,
  synthesizeArguments,
  writeProbeEvidence
} from "../packages/cli/scripts/contract-probe-driver.mjs";
import {
  probeContract,
  probeReportPath,
  resolveProbeRuntime,
  runSessionWithRestarts
} from "../packages/cli/scripts/probe-contract.mjs";

const root = resolve(import.meta.dirname, "..");
const cli = join(root, "packages/cli/bin/solid-checker.mjs");
// `--write` validates the document it is about to install with the native
// checker, exactly as `--promote reviewed` does, and `probeContract` runs
// in-process -- so the launcher override has to be on this process's own
// environment. The checked-in bin/ binary lags rust/ source, which is why the
// debug build is the default here.
const native = process.env.SOLID_CHECKER_NATIVE_BIN ?? join(root, "rust/target/debug/solid-checker-rust");
if (existsSync(native)) process.env.SOLID_CHECKER_NATIVE_BIN = native;

const temporaries = [];
function workspace(prefix = "solid-checker-probe-") {
  const directory = mkdtempSync(join(tmpdir(), prefix));
  temporaries.push(directory);
  return directory;
}
process.on("exit", () => {
  for (const directory of temporaries) rmSync(directory, { recursive: true, force: true });
});

const sha256 = bytes => `sha256:${createHash("sha256").update(bytes).digest("hex")}`;

// Several command-level tests pass `--no-discovery`. That is deliberate and it
// is safe here: the flag only bounds probe time, none of these tests verify the
// contract, and `contract verify` refuses a report produced with it. The one
// test that asserts the flag's own consequence -- that the report records it,
// and that a run without it records discovery as enabled -- is "the probe
// report records the identities the result is a function of".

/// A callback observation with every counter at rest, so each test names only
/// the counters its case is about. `calls` is the measured invocation count the
/// worker now reports; one call is what a site memo that did not re-run makes.
const observation = fields => ({
  ranDuringCall: false,
  runsBeforeWrite: 0,
  runsAfterWrite: 0,
  siteRunsBeforeWrite: 1,
  siteRunsAfterWrite: 1,
  calls: 1,
  ...fields
});

/// A `returns-accessor` observation of a genuine memo accessor: reactive, and
/// cached within one tracked read. Both halves are required -- reactivity alone
/// is satisfied transitively by a closure that forwards to the planted
/// callback.
const accessorObservation = fields => ({
  typeofValue: "function",
  reactive: true,
  trackedReadCalls: 2,
  plantedRunsWithinOneRead: 0,
  calls: 1,
  ...fields
});

test("argument synthesis fills only the slots the contract's own vocabulary names", () => {
  const summary = {
    kind: "function",
    callbacks: [
      { parameter: 0, execution: "tracked" },
      { parameter: 1, execution: "deferred" }
    ],
    reactiveReads: [{ kind: "parameter-member", parameter: 3 }]
  };
  const descriptors = synthesizeArguments(summary, 1);
  assert.deepEqual(descriptors, [
    "noop-callback",
    "probe-callback",
    "undefined",
    "empty-object"
  ]);
  assert.equal(
    descriptors.every(descriptor => ARGUMENT_SYNTHESIS.includes(descriptor)),
    true,
    "the vocabulary is closed: a slot is filled from it or left undefined"
  );
});

test("a slot no contract vocabulary names stays undefined rather than guessed", () => {
  // createProjection(fn, {value: 0}) is RFC 0002's own example of an
  // undrivable call: nothing in schema v1 says what parameter 1 is, so the
  // driver passes undefined and lets the call refuse it, rather than trying
  // {} then [] then 0 until something completes.
  const summary = { kind: "function", callbacks: [{ parameter: 0, execution: "tracked" }] };
  assert.deepEqual(synthesizeArguments(summary, 0), ["probe-callback"]);
});

test("classification reads attribution, not timing", () => {
  assert.equal(
    classifyExecution(observation({ runsBeforeWrite: 1, runsAfterWrite: 2, siteRunsAfterWrite: 2 })),
    "inline",
    "a call site that re-ran owns the reads"
  );
  assert.equal(
    classifyExecution(observation({ runsBeforeWrite: 1, runsAfterWrite: 2 })),
    "tracked",
    "only the callback re-ran, so it holds its own subscription"
  );
  assert.equal(
    classifyExecution(observation({ ranDuringCall: true, runsBeforeWrite: 1, runsAfterWrite: 1 })),
    "inline",
    "synchronous with the listener cleared is still inline"
  );
  assert.equal(
    classifyExecution(observation({ runsBeforeWrite: 1, runsAfterWrite: 1 })),
    "deferred",
    "it ran only after the call returned and holds no subscription"
  );
  assert.equal(classifyExecution(observation({})), null, "a callback that never ran names nothing");
});

const CONTRACT = {
  schemaVersion: 1,
  package: { name: "probe-fixture", version: "1.0.0" },
  compilerFactsProtocol: 1,
  summaries: {
    "function-1": {
      kind: "function",
      callbacks: [{ parameter: 0, execution: "tracked" }],
      returns: { kind: "accessor", label: "memo result" }
    },
    "function-2": {
      kind: "function",
      callbacks: [{ parameter: 0, execution: "inline", owner: "created" }]
    },
    "function-3": { kind: "function", callbacks: { status: "unknown" } },
    "function-4": {
      kind: "function",
      returns: { kind: "store-path", label: "projection result" },
      reactiveReads: [{ kind: "accessor", label: "projection source" }],
      ownerRequirements: [{ operation: "effect" }],
      asyncBehavior: "promise"
    }
  },
  entrypoints: {
    ".": {
      exports: {
        "function-1": ["wrapMemo"],
        "function-2": ["wrapRoot"],
        "function-3": ["opaque"],
        "function-4": ["project"]
      }
    }
  },
  evidence: { kind: "inferred", generator: "solid-checker package generator" }
};

const expanded = () => expandContract(structuredClone(CONTRACT));

test("the plan drives family (B) and records a reason for every undrivable claim", () => {
  const plan = buildProbePlan(expanded(), { modes: PROBE_MODES, discovery: false });
  const claim = name => plan.claims.find(record => record.claim === name && record.export !== undefined);
  const find = (exportName, name) =>
    plan.claims.find(record => record.export === exportName && record.claim === name);

  assert.equal(find("wrapMemo", "callbacks[0]=tracked").family, "B");
  assert.equal(find("wrapMemo", "returns=accessor").family, "B");
  assert.deepEqual(find("wrapMemo", "callbacks[0]=tracked").modesAttempted, [
    "client",
    "server",
    "development",
    "production"
  ]);

  // Permanently out of reach: no observation distinguishes ownership.
  assert.match(find("wrapRoot", "callbacks[0].owner").reason, /owner rows have no probe form/);
  assert.equal(find("wrapRoot", "callbacks[0].owner").family, "C");
  // No claim string, and no evidence slot in schema v1.
  assert.match(find("project", "asyncBehavior").reason, /no evidence slot in schema v1/);
  assert.equal(find("project", "asyncBehavior").family, "C");
  assert.match(find("project", "returns=store-path").reason, /no generic store-path observation/);
  assert.equal(find("project", "returns=store-path").family, "C");

  // The report's family labels agree with what verification does with the row.
  // `reactiveReads[]` and `ownerRequirements[]` are proven from compiler facts
  // and kept, so they are family (A) here -- they used to be reported as (C),
  // the family whose whole definition is "converted before promotion", while
  // verification kept them.
  assert.match(find("project", "ownerRequirements[0]").reason, /static claim about the caller/);
  assert.equal(find("project", "ownerRequirements[0]").family, "A");
  assert.equal(find("project", "reactiveReads[0]").family, "A");
  assert.match(find("project", "reactiveReads[0]").reason, /proven from compiler facts/);

  // An unknown domain states no claim, so there is nothing to drive and
  // nothing to contradict.
  assert.equal(claim("callbacks[0]=inline") !== undefined, true);
  assert.equal(
    plan.claims.some(record => record.export === "opaque" && record.claim.startsWith("callbacks")),
    false
  );
});

test("returns=accessor is undrivable without a callback to plant a signal read in", () => {
  const document = structuredClone(CONTRACT);
  document.summaries["function-1"] = {
    kind: "function",
    returns: { kind: "accessor", label: "memo result" }
  };
  const plan = buildProbePlan(expandContract(document), { discovery: false });
  const record = plan.claims.find(
    claim => claim.export === "wrapMemo" && claim.claim === "returns=accessor"
  );
  assert.equal(record.family, "C");
  assert.match(record.reason, /no plantable reactive source/);
});

test("discovery plants a callback exactly where the contract states none", () => {
  const plan = buildProbePlan(expanded(), { modes: [PROBE_MODES[0]], discovery: true });
  const discovery = plan.sessions[0].probes.filter(probe => probe.type === "discovery");
  const targets = discovery.map(probe => `${probe.export}[${probe.parameter}]`).sort();
  assert.deepEqual(targets, [
    "project[0]",
    "project[1]",
    "wrapMemo[1]",
    "wrapRoot[1]"
  ]);
  assert.equal(
    discovery.some(probe => probe.export === "opaque"),
    false,
    "an unknown callbacks domain states no negative claim, so there is nothing to contradict"
  );
  // The plan says whether discovery ran and over which parameters, because a
  // report that did not say let `contract verify` list the incompleteness
  // blocker as checked when nothing had looked.
  assert.deepEqual(plan.discovery, { enabled: true, parameters: [0, 1] });
  assert.deepEqual(
    buildProbePlan(expanded(), { modes: [PROBE_MODES[0]], discovery: false }).discovery,
    { enabled: false, parameters: [] }
  );
});

test("a value export is probed too: its summary is the maximal negative claim", () => {
  // `kind: value` says the export is not callable at all, and therefore that it
  // invokes no caller-supplied callback anywhere. Discovery used to skip it on
  // the strength of the claim under test.
  const document = structuredClone(CONTRACT);
  document.summaries["value-1"] = { kind: "value" };
  document.entrypoints["."].exports["value-1"] = ["constant"];
  const plan = buildProbePlan(expandContract(document), {
    modes: [PROBE_MODES[0]],
    discovery: true
  });
  const probes = plan.sessions[0].probes.filter(probe => probe.export === "constant");
  assert.deepEqual(probes.map(probe => probe.type).sort(), [
    "discovery",
    "discovery",
    "kind"
  ]);
  // And the kind claim is the falsifier that matters: a `value` that turns out
  // to be callable is a failed claim, which blocks the promotion.
  const { claims } = drive(plan, probe =>
    probe.export === "constant" && probe.type === "kind"
      ? { outcome: "observed", observation: { typeofValue: "function" }, calls: 0 }
      : probe.export === "constant"
        ? { outcome: "not-callable", calls: 0 }
        : trackedAnswer(probe)
  );
  const kind = claims.find(claim => claim.export === "constant" && claim.claim === "kind=value");
  assert.equal(kind.status, "failed");
  assert.match(kind.reason, /runtime kind is function/);
});

/// Drives a plan with canned worker results, keyed by the probe's target.
function drive(plan, answer) {
  const sessions = plan.sessions.map(session => ({
    mode: session.mode,
    results: session.probes.map(probe => ({
      id: probe.id,
      specifier: probe.specifier,
      export: probe.export,
      // The worker starts every result at zero and only a body that invoked the
      // export raises it; each canned answer says what it measured.
      calls: 0,
      ...answer(probe, session.mode)
    }))
  }));
  const incompleteness = [];
  const evidence = [];
  for (const session of sessions) {
    const interpreted = interpretSession({
      claims: plan.claims,
      index: plan.index,
      mode: session.mode,
      results: session.results
    });
    incompleteness.push(...interpreted.incompleteness);
    evidence.push(...interpreted.evidence);
  }
  return { claims: settleClaims(plan.claims), incompleteness, evidence };
}

const trackedAnswer = probe => {
  if (probe.type === "kind") {
    return { outcome: "observed", observation: { typeofValue: "function" }, calls: 0 };
  }
  if (probe.type === "returns-accessor") {
    return { outcome: "observed", observation: accessorObservation(), calls: 1 };
  }
  if (probe.export === "wrapMemo") {
    return {
      outcome: "observed",
      observation: observation({ runsBeforeWrite: 1, runsAfterWrite: 2 }),
      calls: 1
    };
  }
  return { outcome: "threw", error: "TypeError: fn is not a function", calls: 0 };
};

test("a driven claim that matched passes, and a call that threw is undriven, never failed", () => {
  const plan = buildProbePlan(expanded(), { modes: [PROBE_MODES[0]], discovery: false });
  const { claims, incompleteness } = drive(plan, trackedAnswer);
  const find = (exportName, name) =>
    claims.find(claim => claim.export === exportName && claim.claim === name);

  assert.equal(find("wrapMemo", "callbacks[0]=tracked").status, "passed");
  assert.deepEqual(find("wrapMemo", "callbacks[0]=tracked").modesPassed, ["client"]);
  assert.equal(find("wrapMemo", "returns=accessor").status, "passed");
  assert.equal(find("wrapRoot", "callbacks[0]=inline").status, "undriven");
  assert.match(find("wrapRoot", "callbacks[0]=inline").reason, /the synthesized call threw/);
  assert.equal(incompleteness.length, 0);
});

test("a call that completed without invoking the callback is undriven, not a failure", () => {
  const plan = buildProbePlan(expanded(), { modes: [PROBE_MODES[0]], discovery: false });
  const { claims } = drive(plan, probe =>
    probe.type === "callback"
      ? { outcome: "observed", observation: observation({}) }
      : trackedAnswer(probe)
  );
  const record = claims.find(claim => claim.claim === "callbacks[0]=tracked");
  assert.equal(record.status, "undriven");
  assert.match(record.reason, /without invoking the callback/);
});

test("an execution mode the package answered differently is a failure", () => {
  const plan = buildProbePlan(expanded(), { modes: [PROBE_MODES[0]], discovery: false });
  const { claims } = drive(plan, probe =>
    probe.type === "callback" && probe.export === "wrapMemo"
      ? {
          outcome: "observed",
          observation: observation({
            ranDuringCall: true,
            runsBeforeWrite: 1,
            runsAfterWrite: 1
          })
        }
      : trackedAnswer(probe)
  );
  const record = claims.find(claim => claim.claim === "callbacks[0]=tracked");
  assert.equal(record.status, "failed");
  assert.equal(record.reason, "observed inline");
});

test("a mode mismatch the driver's own read scope could explain is undriven, not a failure", () => {
  const plan = buildProbePlan(expanded(), { modes: [PROBE_MODES[0]], discovery: false });
  const { claims } = drive(plan, probe =>
    probe.type === "callback" && probe.export === "wrapMemo"
      ? {
          outcome: "observed",
          observation: observation({
            forcedByAccessorRead: true,
            ranDuringCall: true,
            runsBeforeWrite: 1,
            runsAfterWrite: 1
          })
        }
      : trackedAnswer(probe)
  );
  const record = claims.find(claim => claim.claim === "callbacks[0]=tracked");
  assert.equal(record.status, "undriven");
  assert.match(record.reason, /looked inline rather than tracked/);
});

test("a callable value that never re-read is undriven, and a non-callable one fails the claim", () => {
  const undrivenPlan = buildProbePlan(expanded(), { modes: [PROBE_MODES[0]], discovery: false });
  const { claims: quiet } = drive(undrivenPlan, probe =>
    probe.type === "returns-accessor"
      ? { outcome: "observed", observation: accessorObservation({ reactive: false }) }
      : trackedAnswer(probe)
  );
  const inconclusive = quiet.find(claim => claim.claim === "returns=accessor");
  assert.equal(inconclusive.status, "undriven");
  assert.match(inconclusive.reason, /no re-read followed the planted write/);

  const failingPlan = buildProbePlan(expanded(), { modes: [PROBE_MODES[0]], discovery: false });
  const { claims: wrong } = drive(failingPlan, probe =>
    probe.type === "returns-accessor"
      ? { outcome: "observed", observation: { typeofValue: "object", reactive: false, calls: 1 } }
      : trackedAnswer(probe)
  );
  assert.equal(wrong.find(claim => claim.claim === "returns=accessor").status, "failed");
});

test("a closure that forwards to the planted callback does not satisfy returns=accessor", () => {
  // `(cb) => () => cb()` is reactive by transitivity: the planted signal read
  // lives inside `cb`, so the outer memo re-runs on the write exactly as it
  // does for a real accessor. What separates them is caching -- the forwarding
  // closure re-invokes the callback on every read within one tracked scope.
  const plan = buildProbePlan(expanded(), { modes: [PROBE_MODES[0]], discovery: false });
  const { claims } = drive(plan, probe =>
    probe.type === "returns-accessor"
      ? {
          outcome: "observed",
          observation: accessorObservation({ plantedRunsWithinOneRead: 2 })
        }
      : trackedAnswer(probe)
  );
  const record = claims.find(claim => claim.claim === "returns=accessor");
  assert.equal(record.status, "undriven");
  assert.match(record.reason, /caching could not distinguish/);

  // A runtime that reported no caching measurement at all is undriven too,
  // never passed: an absent measurement is not a passing one.
  const silent = buildProbePlan(expanded(), { modes: [PROBE_MODES[0]], discovery: false });
  const { claims: unmeasured } = drive(silent, probe =>
    probe.type === "returns-accessor"
      ? {
          outcome: "observed",
          observation: { typeofValue: "function", reactive: true, calls: 1 }
        }
      : trackedAnswer(probe)
  );
  const quiet = unmeasured.find(claim => claim.claim === "returns=accessor");
  assert.equal(quiet.status, "undriven");
  assert.match(quiet.reason, /no caching measurement/);
});

test("evidence records the measured call count, not a per-probe-type constant", () => {
  // The export ran once: the site memo did not re-run, which is what `deferred`
  // means. The old table stamped two calls onto exactly this shape.
  const plan = buildProbePlan(expanded(), { modes: [PROBE_MODES[0]], discovery: false });
  const { claims } = drive(plan, probe =>
    probe.export === "wrapMemo" && probe.type === "callback"
      ? {
          outcome: "observed",
          observation: observation({ runsBeforeWrite: 1, runsAfterWrite: 2, calls: 1 }),
          calls: 1
        }
      : trackedAnswer(probe)
  );
  assert.equal(claims.find(claim => claim.claim === "callbacks[0]=tracked").calls, 1);

  const inline = buildProbePlan(expanded(), { modes: [PROBE_MODES[0]], discovery: false });
  const { claims: reran } = drive(inline, probe =>
    probe.export === "wrapMemo" && probe.type === "callback"
      ? {
          outcome: "observed",
          observation: observation({
            runsBeforeWrite: 1,
            runsAfterWrite: 2,
            siteRunsAfterWrite: 1,
            calls: 2
          }),
          calls: 2
        }
      : trackedAnswer(probe)
  );
  assert.equal(reran.find(claim => claim.claim === "callbacks[0]=tracked").calls, 2);

  // A kind observation invokes nothing, so it measures zero calls -- and the
  // report omits a zero rather than printing a call that never happened.
  const kinds = buildProbePlan(expanded(), { modes: [PROBE_MODES[0]], discovery: false });
  const { claims: read } = drive(kinds, trackedAnswer);
  assert.equal(read.find(claim => claim.claim === "kind=function").calls, 0);
});

test("a behavior the contract does not state is an incompleteness finding, never a new row", () => {
  const plan = buildProbePlan(expanded(), { modes: [PROBE_MODES[0]], discovery: true });
  const { claims, incompleteness, evidence } = drive(plan, probe => {
    if (probe.type === "discovery" && probe.export === "wrapRoot" && probe.parameter === 1) {
      return {
        outcome: "observed",
        observation: observation({ runsBeforeWrite: 1, runsAfterWrite: 2 })
      };
    }
    if (probe.type === "discovery") return { outcome: "observed", observation: observation({}) };
    return trackedAnswer(probe);
  });
  assert.equal(incompleteness.length, 1);
  assert.equal(incompleteness[0].claim, "callbacks[1]=tracked");
  assert.match(incompleteness[0].text, /the contract states no such claim/);
  assert.equal(
    claims.some(claim => claim.claim === "callbacks[1]=tracked"),
    false,
    "an observation never becomes a claim"
  );
  assert.equal(
    evidence.some(row => row.claim === "callbacks[1]=tracked"),
    false
  );
});

test("evidence is written only onto claims that already exist, and never over a human's", () => {
  const summary = {
    kind: "function",
    callbacks: [
      { parameter: 0, execution: "tracked" },
      { parameter: 1, execution: "deferred", evidence: { kind: "reviewed" } }
    ],
    returns: { kind: "accessor", label: "memo result" }
  };
  const results = [
    { entrypoint: ".", export: "x", claim: "callbacks[0]=tracked", mode: "client", calls: 2, ok: true },
    { entrypoint: ".", export: "x", claim: "callbacks[0]=tracked", mode: "server", calls: 2, ok: true },
    { entrypoint: ".", export: "x", claim: "callbacks[1]=deferred", mode: "client", calls: 2, ok: true },
    { entrypoint: ".", export: "x", claim: "returns=accessor", mode: "client", calls: 2, ok: true }
  ];
  const next = writeProbeEvidence(summary, results, ".", "x");
  assert.deepEqual(next.callbacks[0].evidence, {
    kind: "probed",
    modes: ["client", "server"],
    calls: 2
  });
  assert.deepEqual(next.callbacks[1].evidence, { kind: "reviewed" });
  assert.equal(next.returns.evidence.kind, "probed");
  assert.equal(next.evidence.kind, "probed");
});

test("a claim that failed in any mode gets no evidence at all", () => {
  const summary = { kind: "function", callbacks: [{ parameter: 0, execution: "tracked" }] };
  const results = [
    { entrypoint: ".", export: "x", claim: "callbacks[0]=tracked", mode: "client", calls: 2, ok: true },
    { entrypoint: ".", export: "x", claim: "callbacks[0]=tracked", mode: "server", calls: 2, ok: false }
  ];
  assert.equal(writeProbeEvidence(summary, results, ".", "x").callbacks[0].evidence, undefined);
});

test("variant evidence is restricted to the modes the variant's conditions resolve under", () => {
  const summary = {
    kind: "function",
    variants: [
      {
        conditions: ["development"],
        summary: { kind: "function", callbacks: [{ parameter: 0, execution: "tracked" }] }
      },
      {
        conditions: ["production"],
        summary: { kind: "function", callbacks: [{ parameter: 0, execution: "inline" }] }
      }
    ]
  };
  const results = [
    {
      entrypoint: ".",
      export: "x",
      claim: "callbacks[0]=tracked",
      mode: "development",
      calls: 2,
      ok: true
    },
    { entrypoint: ".", export: "x", claim: "callbacks[0]=inline", mode: "production", calls: 2, ok: true }
  ];
  const next = writeProbeEvidence(summary, results, ".", "x");
  assert.deepEqual(next.variants[0].summary.callbacks[0].evidence.modes, ["development"]);
  assert.deepEqual(next.variants[1].summary.callbacks[0].evidence.modes, ["production"]);
});

test("applying evidence counts the markers it added", () => {
  const contract = expanded();
  const applied = applyProbeEvidence(contract, [
    { entrypoint: ".", export: "wrapMemo", claim: "callbacks[0]=tracked", mode: "client", calls: 2, ok: true },
    { entrypoint: ".", export: "wrapMemo", claim: "returns=accessor", mode: "client", calls: 2, ok: true }
  ]);
  // The callback row, the return row, and the export summary itself.
  assert.equal(applied.written, 3);
  assert.equal(applied.contract.entrypoints["."].exports.wrapRoot.evidence, undefined);
});

test("a re-driven claim that did not pass loses the marker an earlier run wrote", () => {
  // Supersession. `probed` is durable, so a healthy run's marker survived every
  // later run that observed nothing -- and `contract verify` then certified it.
  // A write now says what *this* run saw, or removes the claim to have seen it.
  const summary = {
    kind: "function",
    callbacks: [
      { parameter: 0, execution: "tracked", evidence: { kind: "probed", modes: ["client", "server"], calls: 2 } },
      { parameter: 1, execution: "deferred", evidence: { kind: "reviewed" } }
    ],
    returns: {
      kind: "accessor",
      label: "memo result",
      evidence: { kind: "probed", modes: ["client"], calls: 1 }
    },
    evidence: { kind: "probed", modes: ["client", "server"], calls: 2 }
  };
  const attempted = attemptedModes([
    { entrypoint: ".", export: "x", claim: "callbacks[0]=tracked", modesAttempted: ["client", "server"] },
    { entrypoint: ".", export: "x", claim: "callbacks[1]=deferred", modesAttempted: ["client"] },
    { entrypoint: ".", export: "x", claim: "returns=accessor", modesAttempted: ["client"] }
  ]);
  const superseded = [];
  const next = writeProbeEvidence(summary, [], ".", "x", PROBE_MODES, { attempted, superseded });

  assert.equal(next.callbacks[0].evidence, undefined, "driven and did not pass: the marker goes");
  assert.deepEqual(next.callbacks[1].evidence, { kind: "reviewed" }, "a human's marker is not ours");
  assert.equal(next.returns.evidence, undefined);
  assert.equal(next.evidence, undefined, "the summary marker covered those claims too");
  assert.deepEqual(
    superseded.map(marker => marker.field).sort(),
    ["callbacks[0]", "evidence", "returns"]
  );

  // A claim this run never attempted keeps what it had: this command reports
  // what it drove, and verification separately refuses to certify a marker its
  // own report does not witness.
  const untouched = writeProbeEvidence(structuredClone(summary), [], ".", "x", PROBE_MODES, {
    attempted: new Map(),
    superseded: []
  });
  assert.deepEqual(untouched.callbacks[0].evidence.modes, ["client", "server"]);
});

test("a narrower re-drive refreshes the marker rather than leaving the wider one", () => {
  const summary = {
    kind: "function",
    callbacks: [
      {
        parameter: 0,
        execution: "tracked",
        evidence: { kind: "probed", modes: ["client", "server", "development", "production"], calls: 2 }
      }
    ]
  };
  const attempted = attemptedModes([
    { entrypoint: ".", export: "x", claim: "callbacks[0]=tracked", modesAttempted: ["client"] }
  ]);
  const next = writeProbeEvidence(
    summary,
    [{ entrypoint: ".", export: "x", claim: "callbacks[0]=tracked", mode: "client", calls: 1, ok: true }],
    ".",
    "x",
    PROBE_MODES,
    { attempted, superseded: [] }
  );
  assert.deepEqual(next.callbacks[0].evidence, { kind: "probed", modes: ["client"], calls: 1 });
});

test("a worker that stopped at a throw is restarted for what is left", () => {
  // Solid 2.0's development build halts the reactive system permanently on an
  // uncaught error, so every observation after a throw in that process is of a
  // runtime where nothing re-runs -- a tracked callback reading as inline. The
  // restart is what keeps that from becoming a false conformance failure.
  const session = {
    mode: "development",
    conditions: ["browser", "development"],
    probes: [{ id: "p1" }, { id: "p2" }, { id: "p3" }, { id: "p4" }]
  };
  const attempts = [];
  const results = runSessionWithRestarts({
    session,
    spawn: probes => {
      attempts.push(probes.map(probe => probe.id));
      const stop = probes[0].id === "p1" ? 1 : probes.length;
      return {
        completed: stop === probes.length,
        results: probes
          .slice(0, stop)
          .map((probe, index) => ({ id: probe.id, outcome: index === stop - 1 && stop === 1 ? "threw" : "observed" }))
      };
    }
  });
  assert.deepEqual(attempts, [
    ["p1", "p2", "p3", "p4"],
    ["p2", "p3", "p4"]
  ]);
  assert.deepEqual(
    results.map(result => result.id),
    ["p1", "p2", "p3", "p4"]
  );
});

test("a crashed or timed-out mode records what is left undriven rather than retrying", () => {
  const session = {
    mode: "client",
    conditions: ["browser"],
    probes: [{ id: "p1" }, { id: "p2" }]
  };
  let attempts = 0;
  const results = runSessionWithRestarts({
    session,
    spawn: () => {
      attempts += 1;
      return { failed: "the probe process was killed by SIGTERM (timeout 60000ms)", results: [] };
    }
  });
  assert.equal(attempts, 1);
  assert.deepEqual(
    results.map(result => result.outcome),
    ["session-failed", "session-failed"]
  );
});

/// A project on disk: an installed package, a solid-js the runtime resolver can
/// classify, and a contract describing the package.
function project({ solidVersion = "1.9.14", contract = CONTRACT, plan = false } = {}) {
  const directory = workspace();
  const modules = join(directory, "node_modules");
  mkdirSync(join(modules, "solid-js"), { recursive: true });
  writeFileSync(
    join(modules, "solid-js", "package.json"),
    JSON.stringify({ name: "solid-js", version: solidVersion })
  );
  mkdirSync(join(modules, "probe-fixture"), { recursive: true });
  writeFileSync(
    join(modules, "probe-fixture", "package.json"),
    JSON.stringify({ name: "probe-fixture", version: contract.package.version })
  );
  const contractDirectory = join(directory, ".solid-checker", "contracts", "probe-fixture");
  mkdirSync(contractDirectory, { recursive: true });
  const contractFile = join(contractDirectory, "solid-reactivity.json");
  writeFileSync(contractFile, `${JSON.stringify(contract, null, 2)}\n`);
  if (plan) {
    writeFileSync(
      join(contractDirectory, "solid-reactivity.review.json"),
      `${JSON.stringify(
        renderReviewPlanDocument(
          contract.package.name,
          contract.package.version,
          collectReviewItems(expandContract(structuredClone(contract)).entrypoints),
          { generator: "solid-checker@test", entrypoints: {} },
          sha256(readFileSync(contractFile))
        ),
        null,
        2
      )}\n`
    );
  }
  return { directory, contractFile, contractDirectory };
}

/// A fake session runner: answers every probe the plan requested.
const fakeSessions = answer => ({ sessions }) =>
  sessions.map(session => ({
    mode: session.mode,
    results: session.probes.map(probe => ({
      id: probe.id,
      specifier: probe.specifier,
      export: probe.export,
      // The worker starts every result at zero and only a body that invoked the
      // export raises it; each canned answer says what it measured.
      calls: 0,
      ...answer(probe, session.mode)
    }))
  }));

test("the probe report records the identities the result is a function of", async () => {
  const { contractFile } = project({ plan: true });
  const report = await probeContract([contractFile, "--no-discovery"], {
    runSessions: fakeSessions(trackedAnswer)
  });
  assert.equal(report.schemaVersion, 1);
  assert.equal(report.package.name, "probe-fixture");
  assert.equal(report.package.installedVersion, "1.0.0");
  assert.equal(report.identities.dialect, "solid-v1");
  assert.deepEqual(report.identities.runtime, { package: "solid-js", version: "1.9.14" });
  assert.equal(report.identities.generator, "solid-checker@test");
  assert.match(report.identities.probeDriver, /^solid-checker@/);
  assert.deepEqual(report.modes, ["client", "server", "development", "production"]);
  assert.equal(report.summary.passed > 0, true);
  assert.equal(report.summary.undriven > 0, true);

  const written = JSON.parse(readFileSync(probeReportPath(contractFile), "utf8"));
  assert.deepEqual(written.summary, report.summary);
  const tracked = written.claims.find(claim => claim.claim === "callbacks[0]=tracked");
  assert.deepEqual(tracked.arguments, ["probe-callback"]);
  assert.deepEqual(tracked.modes.passed, ["client", "development", "production", "server"]);
  const owner = written.claims.find(claim => claim.claim === "callbacks[0].owner");
  assert.equal(owner.family, "C");
  assert.equal(owner.status, "undriven");

  // `--no-discovery` is investigation-only, and the report is where that is
  // visible: `contract verify` refuses a report whose discovery state is
  // anything but enabled, so a run without it can never certify.
  assert.deepEqual(written.discovery, { enabled: false, parameters: [] });
  const complete = await probeContract([contractFile], {
    runSessions: fakeSessions(probe =>
      probe.type === "discovery"
        ? { outcome: "observed", observation: observation({}), calls: 1 }
        : trackedAnswer(probe)
    )
  });
  assert.deepEqual(complete.discovery, { enabled: true, parameters: [0, 1] });
  process.exitCode = 0;
});

test("an evidence write supersedes the marker a claim no longer earns", async () => {
  const { contractFile } = project({ plan: true });
  const healthy = probe =>
    probe.type === "discovery"
      ? { outcome: "observed", observation: observation({}), calls: 1 }
      : trackedAnswer(probe);
  await probeContract([contractFile, "--write"], { runSessions: fakeSessions(healthy) });
  process.exitCode = 0;
  const probed = expandContract(JSON.parse(readFileSync(contractFile, "utf8")));
  assert.equal(probed.entrypoints["."].exports.wrapMemo.callbacks[0].evidence.kind, "probed");

  // The same contract, re-probed against a release that now refuses to load.
  // The old markers used to stay exactly where they were.
  const report = await probeContract([contractFile, "--write"], {
    runSessions: fakeSessions(probe => ({
      outcome: "import-failed",
      specifier: probe.specifier,
      error: "Error: refuses to load in this environment",
      calls: 0
    }))
  });
  process.exitCode = 0;
  const after = expandContract(JSON.parse(readFileSync(contractFile, "utf8")));
  assert.equal(after.entrypoints["."].exports.wrapMemo.callbacks[0].evidence, undefined);
  assert.equal(after.entrypoints["."].exports.wrapMemo.returns.evidence, undefined);
  assert.equal(after.entrypoints["."].exports.wrapMemo.evidence, undefined);
  assert.equal(report.contract.markersSuperseded > 0, true);
  assert.equal(
    report.superseded.some(marker => marker.claim === "callbacks[0]=tracked"),
    true
  );
});

test("a failed probe leaves the contract untouched and exits non-zero", async () => {
  const { contractFile } = project();
  const before = readFileSync(contractFile, "utf8");
  await probeContract([contractFile, "--no-discovery", "--write"], {
    runSessions: fakeSessions(probe =>
      probe.type === "callback" && probe.export === "wrapMemo"
        ? {
            outcome: "observed",
            observation: observation({ ranDuringCall: true, runsBeforeWrite: 1, runsAfterWrite: 1 })
          }
        : trackedAnswer(probe)
    )
  });
  assert.equal(process.exitCode, 1);
  assert.equal(readFileSync(contractFile, "utf8"), before);
  process.exitCode = 0;
});

test("an incompleteness finding blocks the write and exits non-zero", async () => {
  const { contractFile } = project();
  const before = readFileSync(contractFile, "utf8");
  const report = await probeContract([contractFile, "--write"], {
    runSessions: fakeSessions(probe =>
      probe.type === "discovery" && probe.export === "wrapRoot"
        ? {
            outcome: "observed",
            observation: observation({ runsBeforeWrite: 1, runsAfterWrite: 2 })
          }
        : trackedAnswer(probe)
    )
  });
  assert.equal(report.summary.incompleteness > 0, true);
  assert.equal(process.exitCode, 1);
  assert.equal(readFileSync(contractFile, "utf8"), before);
  process.exitCode = 0;
});

test("probing refuses a package whose installed version is not the one the contract describes", async () => {
  const contract = structuredClone(CONTRACT);
  contract.package.version = "2.0.0";
  const { contractFile } = project({ contract });
  const built = project();
  await assert.rejects(
    () =>
      probeContract([contractFile, "--package-root", join(built.directory, "node_modules/probe-fixture")], {
        runSessions: fakeSessions(trackedAnswer)
      }),
    /describes probe-fixture@2\.0\.0 and .* is 1\.0\.0/
  );
});

test("an unclassifiable Solid install refuses instead of falling back to a dialect", () => {
  const { directory } = project({ solidVersion: "0.27.0" });
  assert.throws(
    () => resolveProbeRuntime(join(directory, "node_modules", "probe-fixture")),
    /names no dialect this checker probes/
  );
  const bare = workspace();
  mkdirSync(join(bare, "node_modules"), { recursive: true });
  assert.throws(() => resolveProbeRuntime(bare), /no installed solid-js above/);
});

test("an evidence write re-binds an untouched review plan and refuses a started review", async t => {
  if (!existsSync(native)) {
    t.skip(`no native solid-checker at ${native}`);
    return;
  }
  const { contractFile, contractDirectory } = project({ plan: true });
  const planFile = join(contractDirectory, "solid-reactivity.review.json");
  const before = JSON.parse(readFileSync(planFile, "utf8"));
  await probeContract([contractFile, "--no-discovery", "--write"], {
    runSessions: fakeSessions(trackedAnswer)
  });
  assert.equal(process.exitCode ?? 0, 0);
  const written = JSON.parse(readFileSync(contractFile, "utf8"));
  const summary = expandContract(written).entrypoints["."].exports.wrapMemo;
  assert.equal(summary.callbacks[0].evidence.kind, "probed");
  assert.equal(written.evidence.kind, "inferred", "Stage 1 promotes nothing");
  const report = JSON.parse(readFileSync(probeReportPath(contractFile), "utf8"));
  assert.equal(report.contract.markersWritten, 3);
  assert.notEqual(report.contract.afterWrite, report.contract.hash);
  assert.equal(report.contract.afterWrite, sha256(readFileSync(contractFile)));
  const after = JSON.parse(readFileSync(planFile, "utf8"));
  assert.notEqual(after.contract, before.contract);
  assert.equal(after.contract, sha256(readFileSync(contractFile)));
  assert.deepEqual(
    after.items.map(item => item.id),
    before.items.map(item => item.id),
    "probed evidence raises no new review question"
  );

  // A review that has answered anything binds its decisions to the bytes on
  // disk, so a second write must refuse rather than move them.
  writeFileSync(
    join(contractDirectory, "solid-reactivity.review-state.json"),
    JSON.stringify({
      schemaVersion: 1,
      resolutions: { "some-item": { decision: "confirm", contract: after.contract } }
    })
  );
  const untouched = readFileSync(contractFile, "utf8");
  await assert.rejects(
    () =>
      probeContract([contractFile, "--no-discovery", "--write"], {
        runSessions: fakeSessions(trackedAnswer)
      }),
    /already records 1 review decision/
  );
  assert.equal(readFileSync(contractFile, "utf8"), untouched);
  process.exitCode = 0;
});

test("the CLI dispatches contract probe", () => {
  const child = spawnSync(process.execPath, [cli, "contract", "probe", "--help"], {
    encoding: "utf8"
  });
  assert.equal(child.status ?? 0, 0);
  assert.match(child.stdout, /solid-checker contract probe <CONTRACT>/);
  assert.match(child.stdout, /imports and runs the package's code/);
});

/// The one test that executes a real Solid release.
///
/// It installs the exact version the repository audits into a temporary
/// directory, builds a one-export package around `createMemo`, and drives the
/// tracked-callback claim end to end through the real worker. It skips when the
/// install cannot happen -- offline, or no npm.
test("drives a real tracked-callback claim against an installed Solid release", async t => {
  const directory = workspace("solid-checker-probe-install-");
  writeFileSync(
    join(directory, "package.json"),
    JSON.stringify({ name: "probe-integration", version: "1.0.0", private: true })
  );
  const install = spawnSync(
    "npm",
    ["install", "--prefix", directory, "--no-audit", "--no-fund", "--no-save", "solid-js@1.9.14"],
    { encoding: "utf8", timeout: 300_000 }
  );
  if (install.status !== 0) {
    t.skip(`could not install solid-js@1.9.14: ${(install.stderr ?? install.error?.message ?? "").trim()}`);
    return;
  }
  const packageRoot = join(directory, "node_modules", "probe-fixture");
  mkdirSync(packageRoot, { recursive: true });
  writeFileSync(
    join(packageRoot, "package.json"),
    JSON.stringify({
      name: "probe-fixture",
      version: "1.0.0",
      type: "module",
      exports: { ".": "./index.js" }
    })
  );
  writeFileSync(
    join(packageRoot, "index.js"),
    [
      'import { createMemo, createRoot } from "solid-js";',
      "export const wrapMemo = compute => createMemo(compute);",
      "export const wrapRoot = body => createRoot(() => body());",
      ""
    ].join("\n")
  );
  const contract = {
    schemaVersion: 1,
    package: { name: "probe-fixture", version: "1.0.0" },
    compilerFactsProtocol: 1,
    summaries: {
      "function-1": {
        kind: "function",
        callbacks: [{ parameter: 0, execution: "tracked" }],
        returns: { kind: "accessor", label: "memo result" }
      },
      "function-2": { kind: "function", callbacks: [{ parameter: 0, execution: "inline" }] }
    },
    entrypoints: { ".": { exports: { "function-1": ["wrapMemo"], "function-2": ["wrapRoot"] } } },
    evidence: { kind: "inferred", generator: "solid-checker package generator" }
  };
  const contractFile = join(directory, "solid-reactivity.json");
  writeFileSync(contractFile, `${JSON.stringify(contract, null, 2)}\n`);

  // Every applicable mode, initial and subsequent call, discovery on: the full
  // discipline. `server` is deliberately excluded, and the exclusion is the
  // point -- see the assertion below.
  const report = await probeContract([contractFile, "--modes", "client,development,production"]);
  process.exitCode = 0;

  const tracked = report.claims.find(
    claim => claim.export === "wrapMemo" && claim.claim === "callbacks[0]=tracked"
  );
  assert.equal(tracked.status, "passed", JSON.stringify(tracked));
  assert.deepEqual(tracked.modes.passed, ["client", "development", "production"]);
  // Measured, not tabulated. `wrapMemo` is invoked exactly once: the call-site
  // memo caches, the accessor read happens inside a memo of the driver's own
  // under `untrack`, and so the write re-runs the memo the export created
  // rather than the site. The old constant recorded two calls for this shape.
  assert.equal(tracked.calls, 1);
  const returns = report.claims.find(
    claim => claim.export === "wrapMemo" && claim.claim === "returns=accessor"
  );
  assert.equal(returns.status, "passed", JSON.stringify(returns));
  const inline = report.claims.find(
    claim => claim.export === "wrapRoot" && claim.claim === "callbacks[0]=inline"
  );
  assert.equal(inline.status, "passed", JSON.stringify(inline));
  assert.equal(
    report.claims.find(claim => claim.export === "wrapMemo" && claim.claim === "kind=function").status,
    "passed"
  );
  assert.equal(report.summary.failed, 0);
  assert.equal(
    report.summary.incompleteness,
    0,
    "no callback was invoked at a parameter the contract does not name"
  );

  // Solid 1.x resolves a genuinely different artifact under `node`: a memo
  // computes once and never re-runs, so the callback's own subscription is not
  // there to observe and the driver sees the call site's synchronous run
  // instead. That is a surfaced environment mismatch, and the RFC's rule is
  // that it fails rather than silently narrowing the modes the contract claims.
  const server = await probeContract([contractFile, "--modes", "server", "--no-discovery"]);
  process.exitCode = 0;
  const divergent = server.claims.find(claim => claim.claim === "callbacks[0]=tracked");
  const serverObservation = divergent.observations.find(entry => entry.mode === "server");
  assert.equal(serverObservation.observed, "inline");
  // The claim is *not* asserted as a package defect: the callback ran only
  // because the driver read the accessor the contract states, so which
  // computation owned its reads is partly the driver's own scaffolding.
  assert.equal(divergent.status, "undriven");
  assert.match(serverObservation.reason, /read scope/);

  if (!existsSync(native)) {
    t.diagnostic(`skipped the evidence write: no native solid-checker at ${native}`);
    return;
  }
  await probeContract([contractFile, "--modes", "client,development,production", "--write"]);
  assert.equal(process.exitCode ?? 0, 0);
  const written = expandContract(JSON.parse(readFileSync(contractFile, "utf8")));
  assert.deepEqual(written.entrypoints["."].exports.wrapMemo.callbacks[0].evidence, {
    kind: "probed",
    modes: ["client", "development", "production"],
    calls: 1
  });
  assert.equal(written.evidence.kind, "inferred", "Stage 1 promotes nothing");
});
