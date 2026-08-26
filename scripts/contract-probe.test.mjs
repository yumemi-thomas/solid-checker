// `solid-checker contract probe` -- RFC 0002 Stage 1.
//
// Everything the driver decides is tested hermetically with an injected fake
// runtime: no install, no package code, no native binary. The one test that
// drives a real release is gated on a Bun install succeeding and skips cleanly
// when it cannot.

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  chmodSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { test } from "vitest";

import { installAuditedSolid } from "./lib/audited-solid-runtime.mjs";
import { expandContract } from "../packages/cli/scripts/contract-document.mjs";
import {
  collectReviewItems,
  renderReviewPlanDocument
} from "../packages/cli/scripts/contract-review-plan.mjs";
import {
  ARGUMENT_SYNTHESIS,
  BROWSER_SHIM_GLOBALS,
  EXECUTION_UNATTRIBUTABLE,
  PROBE_MODES,
  applyConstructionPlan,
  applyProbeEvidence,
  attemptedModes,
  buildProbePlan,
  buildProbeReport,
  classifyExecution,
  classifyExecutionResult,
  environmentForMode,
  interpretSession,
  returnClaim,
  returnLeaves,
  settleClaims,
  synthesizeArguments,
  writeProbeEvidence
} from "../packages/cli/scripts/contract-probe-driver.mjs";
import {
  probeContract,
  probeConstructionPlanPath,
  readProbeConstructionPlan,
  probeReportPath,
  resolveProbeRuntime,
  runSessionBySpecifier,
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
let validatorDirectory;
if (existsSync(native)) {
  process.env.SOLID_CHECKER_NATIVE_BIN = native;
} else {
  // Driver tests inject the runtime observations they exercise. A clean CI
  // checkout has no debug checker yet, so keep their write path hermetic too:
  // parse the candidate document, but leave native schema validation to the
  // armed Rust/process and full-gate tests. Falling through to a stale packaged
  // binary made this suite depend on whichever artifact happened to be present.
  validatorDirectory = mkdtempSync(join(tmpdir(), "solid-checker-probe-validator-"));
  const validator = join(validatorDirectory, "solid-checker");
  writeFileSync(
    validator,
    `#!/usr/bin/env bun
import { readFileSync } from "node:fs";
const args = process.argv.slice(2);
const index = args.indexOf("--validate-contract");
if (index === -1 || !args[index + 1]) process.exit(2);
JSON.parse(readFileSync(args[index + 1], "utf8"));
`
  );
  chmodSync(validator, 0o755);
  process.env.SOLID_CHECKER_NATIVE_BIN = validator;
}

const temporaries = [];
function workspace(prefix = "solid-checker-probe-") {
  const directory = mkdtempSync(join(tmpdir(), prefix));
  temporaries.push(directory);
  return directory;
}
process.on("exit", () => {
  for (const directory of temporaries) rmSync(directory, { recursive: true, force: true });
  if (validatorDirectory) rmSync(validatorDirectory, { recursive: true, force: true });
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
///
/// `runsAfterControl` defaults to `runsBeforeWrite`: the control interval is a
/// settle in which nothing was written, and a callback that does not schedule
/// itself runs exactly zero times in it. A case about self-scheduling states it.
const observation = fields => ({
  ranDuringCall: false,
  runsBeforeWrite: 0,
  runsAfterWrite: 0,
  siteRunsBeforeWrite: 1,
  siteRunsAfterWrite: 1,
  calls: 1,
  ...fields,
  runsAfterControl: fields?.runsAfterControl ?? fields?.runsBeforeWrite ?? 0
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
  assert.equal(
    classifyExecution(observation({ runsAfterWrite: 1 })),
    null,
    "a callback that ran for the first time after the write was not re-run by it, and names no mode"
  );
});

test("an observation with no control interval names nothing rather than being read as one", () => {
  // The counters are the tracked shape and the classifier would answer
  // `tracked` for them, which is exactly why the missing field cannot be filled
  // in: reading it as the baseline is the pre-control-interval classifier, and
  // the observations it was wrong about are indistinguishable from this one.
  const result = classifyExecutionResult({
    ranDuringCall: false,
    runsBeforeWrite: 1,
    runsAfterWrite: 2,
    siteRunsBeforeWrite: 1,
    siteRunsAfterWrite: 1,
    calls: 1
  });
  assert.equal(result.execution, null);
  assert.equal(result.reason, EXECUTION_UNATTRIBUTABLE.noControlInterval);
});

/// The counters each real shape produces, measured rather than reasoned about.
///
/// Every row was recorded by running the worker's `callbackObservation` body --
/// control interval included -- against the real release in a temporary
/// directory: solid-js@1.9.14 under `--conditions browser`, cross-checked on
/// 2.0.0-rc.1 (which produces the same counters for the six shapes it can run;
/// it refuses `createEffect` inside a memo, so `inlineAndEffect` throws there and
/// the probe is undriven for a different reason).
///
/// Two of them were recorded through the real worker against the real published
/// package rather than a synthetic stand-in: `@corvu/utils@0.4.2 ./dom:afterPaint`
/// produced `rb 0, rc 1, ra 1, sb 1, sa 1` and `./create/register:default`
/// produced `rb 1, rc 1, ra 3, sb 1, sa 2` -- byte-identical to the rows below.
///
/// `truth` is what the callback's execution mode actually is, and `was` is what
/// the classifier answered before the control interval, the transitive
/// subscription guard and the first-run guard existed. Five rows had `was`
/// wrong.
const EXECUTION_SHAPES = [
  {
    name: "pure inline -- cb => cb()",
    counters: { ranDuringCall: true, runsBeforeWrite: 1, runsAfterWrite: 2, siteRunsAfterWrite: 2 },
    was: "inline",
    is: "inline",
    truth: "inline"
  },
  {
    name: "tracked control -- cb => createMemo(cb)",
    counters: { ranDuringCall: true, runsBeforeWrite: 1, runsAfterWrite: 2 },
    was: "tracked",
    is: "tracked",
    truth: "tracked"
  },
  {
    // @solid-primitives/rootless createSubRoot: createRoot runs its callback
    // synchronously with the listener cleared, which is inline.
    name: "createRoot detach -- cb => createRoot(d => cb(d))",
    counters: { ranDuringCall: true, runsBeforeWrite: 1, runsAfterWrite: 1 },
    was: "inline",
    is: "inline",
    truth: "inline"
  },
  {
    name: "single timer -- cb => setTimeout(cb, 0)",
    counters: { runsBeforeWrite: 1, runsAfterWrite: 1 },
    was: "deferred",
    is: "deferred",
    truth: "deferred"
  },
  {
    // @corvu/utils create/register: mergeProps wraps the function source in a
    // memo, and reading the defaulted member *during the call* subscribes the
    // call site to it. The site re-runs, which used to read as inline; the
    // callback ran twice for that one site re-run, which is the tell.
    name: "read-back memo -- cb => { const m = createMemo(cb); return m() }",
    counters: { ranDuringCall: true, runsBeforeWrite: 1, runsAfterWrite: 3, siteRunsAfterWrite: 2 },
    was: "inline",
    is: null,
    reason: "transitiveSubscription",
    truth: "tracked"
  },
  {
    // @solid-primitives/spring createDerivedSpring and @tanstack/solid-pacer
    // createDebouncedValue: one parameter invoked at two sites with two roles.
    // The contract carries both rows and at most one can be true of one
    // observation, so neither is earned here.
    name: "two roles -- cb => { cb(); createEffect(() => cb()) }",
    counters: { ranDuringCall: true, runsBeforeWrite: 2, runsAfterWrite: 4, siteRunsAfterWrite: 2 },
    was: "inline",
    is: null,
    reason: "transitiveSubscription",
    truth: "inline and tracked"
  },
  {
    // @corvu/utils dom:afterPaint. A double requestAnimationFrame, shimmed to
    // nested timers, so the callback's *first* run lands after the baseline was
    // taken. The write caused nothing at all, and the contract's `deferred` was
    // right.
    name: "late first run -- cb => raf(() => raf(cb))",
    counters: { runsBeforeWrite: 0, runsAfterControl: 1, runsAfterWrite: 1 },
    was: "tracked",
    is: "deferred",
    truth: "deferred"
  },
  {
    // The row above, three milliseconds later. A deferral of roughly three
    // macrotask hops -- `setTimeout(cb, 3)`, a triple `requestAnimationFrame`,
    // a promise chain into a timer -- lands its first run in the *write*
    // interval instead of the control interval, and which one it lands in is a
    // property of the machine's load: measured on solid-js 1.9.14, `setTimeout`
    // at 2ms produced the row above and at 3ms this row, flipping between them
    // on 1 run in 5. The callback had not run by the time of the write, so it
    // had not read the probe's signal and held no subscription to it: the write
    // cannot have caused its first run, and `tracked` reported a plain timeout
    // as a package defect.
    //
    // It is not `deferred` either. The row above earns that reading because the
    // callback ran *before* the write and the write then did not re-run it,
    // which is an observed absence of a subscription. Here there is no such
    // test: a callback whose subscription starts late --
    // `raf(() => raf(() => raf(() => createEffect(cb))))` -- also runs exactly
    // once in the write interval having never run before it, and is genuinely
    // tracked. The counters are identical, so the observation names nothing.
    name: "first run after the write -- cb => setTimeout(cb, 3)",
    counters: { runsBeforeWrite: 0, runsAfterControl: 0, runsAfterWrite: 1 },
    was: "tracked",
    is: null,
    reason: "firstRunAfterWrite",
    truth: "deferred, and counter-identical to a late-tracked callback"
  },
  {
    // @solid-primitives/timer createTimeoutLoop, which reschedules itself, so it
    // runs again across every interval whatever is written and no re-run can be
    // attributed to the write. Its claim was `deferred`, which this cannot
    // confirm -- undriven is the honest answer, not a pass. The counters do not
    // say the callback rescheduled itself: `raf(() => raf(() => createEffect(cb)))`
    // is genuinely tracked and produces the same three counts, which is why the
    // reason names the observation and not a mechanism.
    name: "re-ran with nothing written -- cb => { const t = () => { cb(); setTimeout(t, 0) }; setTimeout(t, 0) }",
    counters: { runsBeforeWrite: 1, runsAfterControl: 2, runsAfterWrite: 3 },
    was: "tracked",
    is: null,
    reason: "unwrittenRerun",
    truth: "deferred"
  }
];

test("the control interval and the attribution guards fix five verdicts and keep four", () => {
  for (const shape of EXECUTION_SHAPES) {
    const result = classifyExecutionResult(observation(shape.counters));
    assert.equal(result.execution, shape.is, shape.name);
    if (shape.is === null) {
      assert.equal(result.reason, EXECUTION_UNATTRIBUTABLE[shape.reason], shape.name);
    }
  }
  // The four that were already right are the four that must not move, and the
  // five that were wrong are the point of the change.
  assert.deepEqual(
    EXECUTION_SHAPES.filter(shape => shape.was === shape.is).map(shape => shape.name),
    EXECUTION_SHAPES.filter(shape => shape.was === shape.truth).map(shape => shape.name)
  );
  assert.equal(EXECUTION_SHAPES.filter(shape => shape.was !== shape.is).length, 5);
});

test("a false verdict is withdrawn as undriven and never turned into a failure", () => {
  // The withdrawal has to reach the claim, not just the classifier: a `tracked`
  // row on the read-back shape used to be reported as a package defect
  // ("observed inline"), and the `inline` row of the two-role shape used to
  // *pass*. Both are now unproven, which is what fails closed means here.
  for (const [shape, claimed] of [
    [EXECUTION_SHAPES[4], "tracked"],
    [EXECUTION_SHAPES[5], "inline"],
    [EXECUTION_SHAPES[8], "deferred"]
  ]) {
    const document = structuredClone(CONTRACT);
    document.summaries["function-1"] = {
      kind: "function",
      callbacks: [{ parameter: 0, execution: claimed }]
    };
    const plan = buildProbePlan(expandContract(document), {
      modes: [PROBE_MODES[0]],
      discovery: false
    });
    const { claims } = drive(plan, probe =>
      probe.type === "callback" && probe.export === "wrapMemo"
        ? { outcome: "observed", observation: observation(shape.counters), calls: 1 }
        : trackedAnswer(probe)
    );
    const record = claims.find(claim => claim.claim === `callbacks[0]=${claimed}`);
    assert.equal(record.status, "undriven", `${shape.name} claimed ${claimed}`);
    assert.equal(record.reason, EXECUTION_UNATTRIBUTABLE[shape.reason], shape.name);
  }
});

test("a late first run confirms the deferred claim it used to contradict", () => {
  // The double-rAF shape is the one false verdict that becomes a *verdict*
  // rather than a withdrawal: the callback ran before the write, so it had read
  // the signal, and nothing ran in the write interval, so the write caused
  // nothing and the contract's `deferred` is confirmed. It used to be reported
  // as a failure against a claim the package honours.
  const document = structuredClone(CONTRACT);
  document.summaries["function-1"] = {
    kind: "function",
    callbacks: [{ parameter: 0, execution: "deferred" }]
  };
  const plan = buildProbePlan(expandContract(document), {
    modes: [PROBE_MODES[0]],
    discovery: false
  });
  const { claims } = drive(plan, probe =>
    probe.type === "callback" && probe.export === "wrapMemo"
      ? {
          outcome: "observed",
          observation: observation(EXECUTION_SHAPES[6].counters),
          calls: 1
        }
      : trackedAnswer(probe)
  );
  const record = claims.find(claim => claim.claim === "callbacks[0]=deferred");
  assert.equal(record.status, "passed");
  assert.deepEqual(record.modesPassed, ["client"]);
});

test("a first run in the write interval names no mode in either direction", () => {
  // The same shape one macrotask later, at the claim level rather than the
  // classifier's. `setTimeout(cb, 3)` used to answer `tracked`, so a package
  // stating `deferred` was reported as defective ("observed tracked") and a
  // package stating `tracked` *passed* on the strength of a run the write
  // cannot have caused. Neither row is earned now: the counters a
  // late-subscribing callback produces are the same ones, so both claims are
  // withdrawn rather than one of them being certified by the coin flip.
  for (const claimed of ["deferred", "tracked"]) {
    const document = structuredClone(CONTRACT);
    document.summaries["function-1"] = {
      kind: "function",
      callbacks: [{ parameter: 0, execution: claimed }]
    };
    const plan = buildProbePlan(expandContract(document), {
      modes: [PROBE_MODES[0]],
      discovery: false
    });
    const { claims, evidence } = drive(plan, probe =>
      probe.type === "callback" && probe.export === "wrapMemo"
        ? {
            outcome: "observed",
            observation: observation(EXECUTION_SHAPES[7].counters),
            calls: 1
          }
        : trackedAnswer(probe)
    );
    const record = claims.find(claim => claim.claim === `callbacks[0]=${claimed}`);
    assert.equal(record.status, "undriven", claimed);
    assert.equal(record.reason, EXECUTION_UNATTRIBUTABLE.firstRunAfterWrite, claimed);
    assert.equal(
      evidence.some(row => row.claim === `callbacks[0]=${claimed}`),
      false,
      `${claimed} contributes no evidence, passing or failing`
    );
  }
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

test("relational returns are family B probes whose claim identity includes the parameter", () => {
  const document = {
    schemaVersion: 1,
    package: { name: "relations", version: "1.0.0" },
    compilerFactsProtocol: 1,
    summaries: {
      argument: { kind: "function", returns: { kind: "argument", parameter: 1 } },
      callback: { kind: "function", returns: { kind: "callback-result", parameter: 0 } },
      factory: {
        kind: "function",
        returns: { kind: "callback-result-function", parameter: 2 }
      }
    },
    entrypoints: {
      ".": { exports: { argument: ["identity"], callback: ["call"], factory: ["factory"] } }
    },
    evidence: { kind: "inferred", generator: "test" }
  };
  const plan = buildProbePlan(expandContract(document), {
    modes: [PROBE_MODES[0]],
    discovery: false
  });
  const relations = plan.claims.filter(claim => claim.claim.startsWith("returns="));
  assert.deepEqual(
    relations.map(claim => [claim.export, claim.claim, claim.family]),
    [
      ["identity", "returns=argument[1]", "B"],
      ["call", "returns=callback-result[0]", "B"],
      ["factory", "returns=callback-result-function[2]", "B"]
    ]
  );
  assert.equal(returnClaim({ kind: "argument", parameter: 0 }), "returns=argument[0]");
  const probes = plan.sessions[0].probes.filter(probe => probe.type.startsWith("returns-"));
  assert.deepEqual(
    probes.map(probe => [probe.type, probe.parameter, probe.arguments]),
    [
      ["returns-argument", 1, ["undefined", "probe-value"]],
      ["returns-callback-result", 0, ["probe-callback"]],
      ["returns-callback-result-function", 2, ["undefined", "undefined", "probe-callback"]]
    ]
  );

  const passed = drive(plan, probe =>
    probe.type === "kind"
      ? { outcome: "observed", observation: { typeofValue: "function" } }
      : {
          outcome: "observed",
          observation: {
            returnedType: probe.type.endsWith("function") ? "function" : "object",
            invocationResultType: "object",
            identityMatched: true,
            calls: 1
          },
          calls: 1
        }
  );
  for (const claim of passed.claims.filter(claim => claim.claim.startsWith("returns="))) {
    assert.equal(claim.status, "passed", claim.claim);
  }
});

test("a completed relational return that breaks strict identity is a witnessed failure", () => {
  const document = structuredClone(CONTRACT);
  document.summaries["function-1"] = {
    kind: "function",
    returns: { kind: "callback-result-function", parameter: 0 }
  };
  const plan = buildProbePlan(expandContract(document), {
    modes: [PROBE_MODES[0]],
    discovery: false
  });
  const { claims } = drive(plan, probe =>
    probe.type === "kind"
      ? { outcome: "observed", observation: { typeofValue: "function" } }
      : probe.type === "returns-callback-result-function"
        ? {
            outcome: "observed",
            observation: {
              returnedType: "function",
              invocationResultType: "object",
              identityMatched: false,
              calls: 1
            },
            calls: 1
          }
        : { outcome: "threw", error: "not relevant" }
  );
  const relation = claims.find(claim => claim.claim === "returns=callback-result-function[0]");
  assert.equal(relation.status, "failed");
  assert.match(relation.reason, /did not return the planted callback value by identity/);
});

test("nested return leaves have path-bound claim identities and probes", () => {
  const document = structuredClone(CONTRACT);
  document.summaries["function-1"] = {
    kind: "function",
    callbacks: [{ parameter: 0, execution: "tracked" }],
    returns: {
      kind: "tuple",
      elements: [
        { kind: "accessor", label: "first" },
        { kind: "object", properties: { 'odd.name': { kind: "argument", parameter: 1 } } }
      ]
    }
  };
  document.entrypoints["."].exports = { "function-1": ["nested"] };
  const expanded = expandContract(document);
  const plan = buildProbePlan(expanded, { modes: [PROBE_MODES[0]], discovery: false });
  const claims = plan.claims.filter(claim => claim.claim.startsWith("returns"));
  assert.deepEqual(
    claims.map(claim => claim.claim),
    [
      "returns.elements[0]=accessor",
      'returns.elements[1].properties["odd.name"]=argument[1]'
    ]
  );
  assert.deepEqual(
    plan.sessions[0].probes.filter(probe => probe.type.startsWith("returns-")).map(probe => probe.returnPath),
    [[0], [1, "odd.name"]]
  );
  assert.deepEqual(returnLeaves(expanded.entrypoints["."].exports.nested.returns).map(leaf => leaf.path), [[0], [1, "odd.name"]]);
});

test("TypeFacts construction recipes fill only otherwise-undefined slots", () => {
  assert.deepEqual(
    applyConstructionPlan(
      ["undefined", "probe-callback", "empty-object", "undefined"],
      { 0: "null", 1: "empty-array", 2: "empty-map", 3: "empty-set", 9: "null" }
    ),
    ["null", "probe-callback", "empty-object", "empty-set"]
  );
});

test("probe construction plans are bound to exact contract bytes", () => {
  const directory = workspace("solid-checker-construction-plan-");
  const contractFile = join(directory, "solid-reactivity.json");
  const bytes = Buffer.from("{}\n");
  writeFileSync(contractFile, bytes);
  const hash = sha256(bytes);
  const path = probeConstructionPlanPath(contractFile);
  writeFileSync(
    path,
    `${JSON.stringify({
      schemaVersion: 1,
      contract: hash,
      source: "typescript-value-domain",
      package: { name: "x", version: "1" },
      entrypoints: { ".": { f: { 0: "null" } } }
    })}\n`
  );
  assert.equal(
    readProbeConstructionPlan(contractFile, hash, { name: "x", version: "1" }).entrypoints["."].f[0],
    "null"
  );
  assert.throws(
    () => readProbeConstructionPlan(contractFile, "sha256:stale", { name: "x", version: "1" }),
    /regenerate before probing/
  );
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
      // The runtime capability the worker stamps on every driven observation.
      // These canned answers stand for a reactive runtime, which is what an
      // ordinary session has; the tests about an inert one stamp it themselves.
      runtime: { reruns: true },
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

// ---------------------------------------------------------------------------
// The runtime capability self-check
// ---------------------------------------------------------------------------

/// Every observation of a session, stamped with one runtime capability.
const withRuntime = (reruns, answer) => probe => {
  const result = answer(probe);
  return result.outcome === "observed" ? { ...result, runtime: { reruns } } : result;
};

test("a runtime that re-runs nothing observes no execution mode, in either direction", () => {
  // The whole of RC1. Both audited releases resolve `node` to a server build
  // where `createSignal` is a constant, `createMemo` computes once and
  // `createEffect` is empty -- so the probe's own scaffolding is inert. A
  // `tracked` claim cannot be observed there and used to be reported as a
  // package defect; an `inline` or `deferred` claim cannot be observed there
  // either and used to *pass*, which is the half that matters, because a pass
  // becomes probed row evidence and then a verified contract.
  for (const claimed of ["tracked", "inline", "deferred"]) {
    const document = structuredClone(CONTRACT);
    document.summaries["function-1"] = {
      kind: "function",
      callbacks: [{ parameter: 0, execution: claimed }]
    };
    const plan = buildProbePlan(expandContract(document), {
      modes: [PROBE_MODES[0]],
      discovery: false
    });
    const { claims, evidence } = drive(
      plan,
      withRuntime(false, probe =>
        probe.type === "callback" && probe.export === "wrapMemo"
          ? {
              outcome: "observed",
              // The counters an inert runtime produces for every shape: the
              // callback ran once, during the call, and nothing ever re-ran.
              observation: observation({ ranDuringCall: true, runsBeforeWrite: 1, runsAfterWrite: 1 }),
              calls: 1
            }
          : trackedAnswer(probe)
      )
    );
    const record = claims.find(claim => claim.claim === `callbacks[0]=${claimed}`);
    assert.equal(record.status, "undriven", claimed);
    assert.equal(record.reason, EXECUTION_UNATTRIBUTABLE.runtimeInert, claimed);
    assert.equal(
      evidence.some(row => row.claim === `callbacks[0]=${claimed}`),
      false,
      `${claimed} contributes no evidence, passing or failing`
    );
  }
});

test("an inert runtime still observes kind, which reads typeof and needs no reactivity", () => {
  const plan = buildProbePlan(expanded(), { modes: [PROBE_MODES[0]], discovery: false });
  const { claims } = drive(plan, withRuntime(false, trackedAnswer));
  assert.equal(claims.find(claim => claim.claim === "kind=function").status, "passed");
});

/// What a `returns=accessor` claim can answer in an inert mode, over the two
/// observations such a mode can actually produce.
///
/// A `returns` claim keeps its verdict rather than being withdrawn with the
/// callback claims, and these are the two halves of why. `reactive: true` is not
/// among them: it requires a re-read after the write, which is the one thing an
/// inert runtime cannot produce, so a case that stamps `reruns: false` and feeds
/// `reactive: true` documents the exemption with an input no inert mode can
/// generate. `typeof` is what an inert runtime can still observe, and a returned
/// non-function is a real contradiction of `accessor` whatever re-runs.
const INERT_RETURNS = [
  {
    name: "a callable return that never re-read is undriven, not a pass",
    observation: { typeofValue: "function", reactive: false, calls: 1 },
    status: "undriven",
    reason: /no re-read followed the planted write/
  },
  {
    name: "a returned non-function contradicts accessor even where nothing re-runs",
    observation: { typeofValue: "object", reactive: false, calls: 1 },
    status: "failed",
    reason: /which cannot be an accessor/
  }
];

test("a returns claim in an inert mode answers from typeof, and never from reactivity", () => {
  for (const row of INERT_RETURNS) {
    const plan = buildProbePlan(expanded(), { modes: [PROBE_MODES[0]], discovery: false });
    const { claims } = drive(
      plan,
      withRuntime(false, probe =>
        probe.type === "returns-accessor"
          ? { outcome: "observed", observation: row.observation, calls: 1 }
          : trackedAnswer(probe)
      )
    );
    const record = claims.find(claim => claim.claim === "returns=accessor");
    assert.equal(record.status, row.status, row.name);
    assert.match(record.reason, row.reason, row.name);
  }
});

test("an unstamped observation is undriven, because an unasked runtime is not a re-running one", () => {
  const plan = buildProbePlan(expanded(), { modes: [PROBE_MODES[0]], discovery: false });
  const { claims } = drive(plan, probe =>
    probe.type === "callback" && probe.export === "wrapMemo"
      ? {
          outcome: "observed",
          observation: observation({ runsBeforeWrite: 1, runsAfterWrite: 2 }),
          calls: 1,
          runtime: undefined
        }
      : trackedAnswer(probe)
  );
  const record = claims.find(claim => claim.claim === "callbacks[0]=tracked");
  assert.equal(record.status, "undriven");
  assert.equal(record.reason, EXECUTION_UNATTRIBUTABLE.runtimeInert);
});

test("an inert runtime reports no incompleteness, because the mode in the finding would be invented", () => {
  const plan = buildProbePlan(expanded(), { modes: [PROBE_MODES[0]], discovery: true });
  const answer = probe => {
    if (probe.type === "discovery" && probe.export === "wrapRoot" && probe.parameter === 1) {
      return {
        outcome: "observed",
        observation: observation({ ranDuringCall: true, runsBeforeWrite: 1, runsAfterWrite: 1 })
      };
    }
    if (probe.type === "discovery") return { outcome: "observed", observation: observation({}) };
    return trackedAnswer(probe);
  };
  assert.equal(drive(plan, withRuntime(false, answer)).incompleteness.length, 0);
  // The same observation in a re-running runtime is a finding, so the
  // suppression is about attribution and not about discovery being off.
  const reactive = buildProbePlan(expanded(), { modes: [PROBE_MODES[0]], discovery: true });
  const found = drive(reactive, withRuntime(true, answer)).incompleteness;
  assert.equal(found.length, 1);
  assert.equal(found[0].claim, "callbacks[1]=inline");
});

test("the capability is per runtime, not per session: one session can hold both answers", () => {
  // Measured, not hypothesised. Probing solid-js@1.9.14 under `--conditions
  // node`, `import "solid-js"` resolves to `dist/server.js` and re-runs nothing,
  // while `import "solid-js/jsx-dev-runtime"` resolves to `dist/solid.js` -- the
  // manifest gives that subpath one unconditional target -- satisfies the
  // worker's `drivesItself` check and re-runs normally. A per-session answer is
  // wrong about one of the two whichever runtime it is taken from.
  const document = structuredClone(CONTRACT);
  document.summaries["function-1"] = {
    kind: "function",
    callbacks: [{ parameter: 0, execution: "tracked" }]
  };
  document.summaries["function-2"] = {
    kind: "function",
    callbacks: [{ parameter: 0, execution: "tracked" }]
  };
  const plan = buildProbePlan(expandContract(document), {
    modes: [PROBE_MODES[1]],
    discovery: false
  });
  const { claims } = drive(plan, probe => {
    if (probe.type !== "callback") return trackedAnswer(probe);
    const tracked = {
      outcome: "observed",
      observation: observation({ ranDuringCall: true, runsBeforeWrite: 1, runsAfterWrite: 2 }),
      calls: 1
    };
    // `wrapMemo` stands for the entrypoint the inert artifact resolved;
    // `wrapRoot` for the one whose own artifact drives it.
    return probe.export === "wrapMemo"
      ? { ...tracked, observation: observation({ ranDuringCall: true, runsBeforeWrite: 1, runsAfterWrite: 1 }), runtime: { reruns: false } }
      : { ...tracked, runtime: { reruns: true } };
  });
  const inert = claims.find(claim => claim.export === "wrapMemo" && claim.claim === "callbacks[0]=tracked");
  const driven = claims.find(claim => claim.export === "wrapRoot" && claim.claim === "callbacks[0]=tracked");
  assert.equal(inert.status, "undriven");
  assert.equal(inert.reason, EXECUTION_UNATTRIBUTABLE.runtimeInert);
  assert.equal(driven.status, "passed", "a reactive runtime in the same session is still driven");
  assert.deepEqual(driven.modesPassed, ["server"]);
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

test("nested return evidence is written only to the exact leaf", () => {
  const summary = {
    kind: "function",
    returns: {
      kind: "tuple",
      elements: [{ kind: "argument", parameter: 0 }, { kind: "argument", parameter: 1 }]
    }
  };
  const results = [{
    entrypoint: ".",
    export: "x",
    claim: "returns.elements[1]=argument[1]",
    mode: "client",
    calls: 1,
    ok: true
  }];
  const next = writeProbeEvidence(summary, results, ".", "x");
  assert.equal(next.returns.elements[0].evidence, undefined);
  assert.equal(next.returns.elements[1].evidence.kind, "probed");
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
  const { results, accounting } = runSessionWithRestarts({
    session,
    spawn: probes => {
      attempts.push(probes.map(probe => probe.id));
      const stop = probes[0].id === "p1" ? 1 : probes.length;
      return {
        completed: stop === probes.length,
        // The worker measures the runtime it will drive probes with and reports
        // the answer per session. Every process of a mode re-imports the same
        // artifacts, so the first answer is the mode's answer.
        runtime: { reruns: true },
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
  // The restart is now accounted rather than only inferable from wall time.
  assert.deepEqual(accounting, {
    mode: "development",
    started: 2,
    restarts: 1,
    failed: 0,
    completed: true,
    runtime: { reruns: true }
  });
});

test("an entrypoint import failure cannot suppress a different entrypoint", () => {
  const session = {
    mode: "server",
    conditions: ["node"],
    probes: [
      { id: "bad", specifier: "pkg/browser-only" },
      { id: "good", specifier: "pkg/server-safe" }
    ]
  };
  const attempts = [];
  const { results, accounting } = runSessionBySpecifier({
    session,
    spawn: probes => {
      attempts.push(probes.map(probe => probe.specifier));
      if (probes[0].specifier === "pkg/browser-only") {
        return {
          completed: false,
          runtime: { reruns: false },
          results: [{ id: "bad", outcome: "import-failed" }]
        };
      }
      return {
        completed: true,
        runtime: { reruns: false },
        results: [{ id: "good", outcome: "observed" }]
      };
    }
  });
  assert.deepEqual(attempts, [["pkg/browser-only"], ["pkg/server-safe"]]);
  assert.deepEqual(results.map(result => result.outcome), ["import-failed", "observed"]);
  assert.deepEqual(accounting, {
    mode: "server",
    started: 2,
    restarts: 0,
    failed: 0,
    completed: true,
    runtime: { reruns: false }
  });
});

test("a crashed or timed-out mode records what is left undriven rather than retrying", () => {
  const session = {
    mode: "client",
    conditions: ["browser"],
    probes: [{ id: "p1" }, { id: "p2" }]
  };
  let attempts = 0;
  const { results, accounting } = runSessionWithRestarts({
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
  // A mode whose only process died measured no runtime, and says so rather than
  // reporting the inert answer it never took.
  assert.deepEqual(accounting, {
    mode: "client",
    started: 1,
    restarts: 0,
    failed: 1,
    completed: false,
    runtime: null
  });
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
      // The runtime capability the worker stamps on every driven observation.
      // These canned answers stand for a reactive runtime, which is what an
      // ordinary session has; the tests about an inert one stamp it themselves.
      runtime: { reruns: true },
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
/// install cannot happen -- offline, or no Bun.
test("drives a real tracked-callback claim against an installed Solid release", async t => {
  const directory = workspace("solid-checker-probe-install-");
  writeFileSync(
    join(directory, "package.json"),
    JSON.stringify({ name: "probe-integration", version: "1.0.0", private: true })
  );
  const install = installAuditedSolid(directory);
  if (!install.ok) {
    t.skip(`could not install solid-js@1.9.14: ${install.message ?? "the cached runtime was unavailable"}`);
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
  // computes once and never re-runs, `createEffect` is empty, and a signal is a
  // constant. The probe's own scaffolding is inert there, so `tracked` is not
  // merely unobserved but *unobservable* -- and so are `inline` and `deferred`,
  // which is why this had to withdraw a pass as well as a failure.
  //
  // The runtime self-check is what turns that into a fact about the run rather
  // than a fact about the package: the session's runtime is asked whether it
  // re-runs anything, it answers no, and every callback observation of that
  // runtime becomes undriven. Nothing here names the mode, the condition or the
  // artifact; a `server` session that resolved a reactive artifact -- which
  // `solid-js/jsx-dev-runtime` does, unconditionally -- is driven normally.
  const server = await probeContract([contractFile, "--modes", "server", "--no-discovery"]);
  process.exitCode = 0;
  for (const claimed of ["callbacks[0]=tracked", "callbacks[0]=inline"]) {
    const claim = server.claims.find(record => record.claim === claimed);
    const seen = claim.observations.find(entry => entry.mode === "server");
    assert.equal(claim.status, "undriven", claimed);
    assert.equal(seen.status, "undriven", claimed);
    assert.equal(seen.observed, undefined, `${claimed} names no mode it could not observe`);
    assert.equal(seen.reason, EXECUTION_UNATTRIBUTABLE.runtimeInert, claimed);
  }
  assert.equal(server.summary.failed, 0, "an unobservable claim is never a package defect");
  assert.equal(server.summary.passed, 2, "only the two kind claims, which read typeof");

  if (!existsSync(native)) {
    console.info(`skipped the evidence write: no native solid-checker at ${native}`);
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

// ---------------------------------------------------------------------------
// The import environment
// ---------------------------------------------------------------------------

test("only browser-condition modes get a shim, and server never does", () => {
  // The whole honesty of the shim rests on this line. A `server` import that
  // throws on `window` is a truthful observation of that entrypoint under
  // `--conditions node`; faking a DOM there would manufacture a pass.
  const byName = Object.fromEntries(PROBE_MODES.map(mode => [mode.name, mode]));
  for (const name of ["client", "development", "production"]) {
    const environment = environmentForMode(byName[name]);
    assert.equal(environment.kind, "browser-globals");
    assert.deepEqual(environment.globals, [...BROWSER_SHIM_GLOBALS]);
  }
  const server = environmentForMode(byName.server);
  assert.equal(server.kind, "none");
  assert.deepEqual(server.globals, []);
});

test("the shim can be switched off, which is the environment every earlier run had", () => {
  for (const mode of PROBE_MODES) {
    assert.deepEqual(environmentForMode(mode, { shim: false }), { kind: "none", globals: [] });
  }
});

test("every session of a plan carries the environment its mode resolves to", () => {
  const plan = buildProbePlan(expandContract(CONTRACT));
  assert.ok(plan.sessions.length > 0);
  for (const session of plan.sessions) {
    assert.equal(
      session.environment.kind,
      session.mode === "server" ? "none" : "browser-globals",
      `session ${session.mode}`
    );
  }
  const bare = buildProbePlan(expandContract(CONTRACT), { environmentShim: false });
  for (const session of bare.sessions) assert.equal(session.environment.kind, "none");
});

/// Runs the real worker in a staged directory, exactly as `defaultRunSessions`
/// does, against a package whose module body reports what the environment
/// looked like while it was being evaluated.
function runWorker({
  body,
  environment,
  mode = "client",
  probeCount = 1,
  probes,
  solid = "export const createSignal = () => [];\n",
  packages = {}
}) {
  const directory = workspace("solid-checker-worker-");
  const modules = join(directory, "node_modules");
  const install = (name, source, version = "1.0.0") => {
    mkdirSync(join(modules, name), { recursive: true });
    writeFileSync(
      join(modules, name, "package.json"),
      JSON.stringify({ name, version, type: "module", main: "index.mjs" })
    );
    writeFileSync(join(modules, name, "index.mjs"), source);
  };
  // The worker builds its probe runtime from `solid-js` before it touches any
  // probe, so a staging directory without one answers every probe "no probe
  // runtime" and tests nothing about the environment. The default stub only has
  // to exist -- the environment tests drive no reactive claim -- and a test
  // about the runtime capability supplies a runtime of its own.
  install("solid-js", solid, "1.9.14");
  if (body !== undefined) install("env-fixture", body);
  for (const [name, source] of Object.entries(packages)) install(name, source);
  const worker = join(modules, "contract-probe-worker.mjs");
  writeFileSync(
    worker,
    readFileSync(join(root, "packages/cli/scripts/contract-probe-worker.mjs"), "utf8")
  );
  const requestFile = join(directory, "request.json");
  writeFileSync(
    requestFile,
    JSON.stringify({
      mode,
      dialect: "solid-v1",
      environment,
      probes:
        probes ??
        Array.from({ length: probeCount }, (_, index) => ({
          id: `p${index + 1}`,
          type: "kind",
          specifier: "env-fixture",
          export: "report"
        }))
    })
  );
  const child = spawnSync(process.execPath, [worker, requestFile], {
    cwd: directory,
    encoding: "utf8"
  });
  assert.equal(child.status, 0, child.stderr);
  return JSON.parse(child.stdout);
}

test("the worker establishes all three relational returns with a fresh identity sentinel", () => {
  const solid = [
    "export const createSignal = value => [() => value, next => { value = next }];",
    "export const createMemo = body => body;",
    "export const untrack = body => body();",
    "export const createRoot = body => body(() => {});"
  ].join("\n");
  const body = [
    "export const argument = (_unused, value) => value;",
    "export const callback = fn => fn();",
    "export const factory = (_a, _b, fn) => () => fn();",
    "export const wrong = fn => () => ({ value: fn() });",
    "export const nested = (_unused, value) => [0, { value }];"
  ].join("\n");
  const probe = (id, type, exportName, parameter, arguments_) => ({
    id,
    type,
    specifier: "env-fixture",
    export: exportName,
    parameter,
    arguments: arguments_
  });
  const answer = runWorker({
    body,
    solid,
    environment: { kind: "none", globals: [] },
    probes: [
      probe("p1", "returns-argument", "argument", 1, ["undefined", "probe-value"]),
      probe("p2", "returns-callback-result", "callback", 0, ["probe-callback"]),
      probe(
        "p3",
        "returns-callback-result-function",
        "factory",
        2,
        ["undefined", "undefined", "probe-callback"]
      ),
      probe("p4", "returns-callback-result-function", "wrong", 0, ["probe-callback"]),
      { ...probe("p5", "returns-argument", "nested", 1, ["undefined", "probe-value"]), returnPath: [1, "value"] }
    ]
  });
  assert.deepEqual(
    answer.results.map(result => [result.export, result.observation.identityMatched]),
    [
      ["argument", true],
      ["callback", true],
      ["factory", true],
      ["wrong", false],
      ["nested", true]
    ]
  );
  assert.equal(answer.results[2].observation.returnedFunctionCalls, 1);
  assert.equal(answer.results[3].observation.callbackCalls, 1);
});

test("the worker defines the requested globals before it imports anything", () => {
  // The failure the shim exists for is a *module-evaluation-time* dereference,
  // so the fixture reads `window` bare at the top level of its own body --
  // the position that throws `ReferenceError` in a bare Node process.
  const body = `const seen = { width: window.innerWidth, ready: document.readyState };\nexport const report = seen;\n`;
  const shimmed = runWorker({
    body,
    environment: { kind: "browser-globals", globals: ["window", "document"] }
  });
  assert.equal(shimmed.results[0].outcome, "observed");
  assert.deepEqual(shimmed.environment, {
    kind: "browser-globals",
    shimmed: ["window", "document"],
    present: []
  });

  const bare = runWorker({ body, environment: { kind: "none", globals: [] } });
  assert.equal(bare.results[0].outcome, "import-failed");
  assert.match(bare.results[0].error, /window is not defined/);
  assert.deepEqual(bare.environment, { kind: "none", shimmed: [], present: [] });
});

test("a typeof guard never threw, so the shim changes which branch it takes", () => {
  // The honest half of the premise. `typeof window` is legal on an undeclared
  // identifier and never threw, so the shim buys nothing for a module that
  // guards that way -- it *redirects* it. A package that took its server path
  // in every earlier measurement now takes its browser path, and what is then
  // observed is behavior given a fake DOM. This is exactly why the shimmed
  // list is recorded rather than assumed harmless.
  const body = "export const report = typeof window === 'undefined' ? 'server' : 'browser';";
  const bare = runWorker({ body, environment: { kind: "none", globals: [] } });
  assert.equal(bare.results[0].outcome, "observed");
  const shimmed = runWorker({
    body,
    environment: { kind: "browser-globals", globals: ["window"] }
  });
  assert.equal(shimmed.results[0].outcome, "observed");
  assert.deepEqual(shimmed.environment.shimmed, ["window"]);
});

test("a shimmed value admits that it is one, and the record is readable from inside", () => {
  // Inert-observable: a probe body that ever needs to know the DOM was fake
  // must be able to find out. Both markers are non-enumerable accessors, so a
  // package's own feature detection sees exactly what a browser would.
  const body = [
    "export const report = {",
    "  windowIsShim: window.__solidCheckerProbeShim === true,",
    "  documentIsShim: window.document.__solidCheckerProbeShim === true,",
    "  record: globalThis.__solidCheckerProbeEnvironment.shimmed,",
    "  selfIsWindow: self === window,",
    "  documentIsSameObject: window.document === document,",
    "  markerHidden: Object.keys(window).includes('__solidCheckerProbeShim'),",
    "  windowInGlobal: 'window' in globalThis",
    "};"
  ].join("\n");
  const answer = runWorker({
    body,
    environment: { kind: "browser-globals", globals: ["window", "self", "document"] }
  });
  assert.equal(answer.results[0].outcome, "observed");
  assert.deepEqual(
    answer.environment.shimmed,
    process.versions.bun ? ["window", "document"] : ["window", "self", "document"]
  );
});

test("the fake DOM closes the back-references a real one has", () => {
  // Not decoration. A package that reaches `node.ownerDocument.addEventListener`
  // from a *deferred* callback throws inside a timer, which is an uncaught
  // exception that kills the worker process rather than one probe -- and takes
  // every remaining claim of that mode, `kind` observations included, with it.
  // Two corpus rows were lost exactly that way.
  const body = [
    "const node = document.createElement('div');",
    "export const report = {",
    "  ownerDocumentIsDocument: node.ownerDocument === document,",
    "  defaultViewIsWindow: document.defaultView === window,",
    "  listenerRegisters: (node.ownerDocument.addEventListener('x', () => {}), true),",
    "  bodyOwnerDocument: document.body.ownerDocument === document",
    "};"
  ].join("\n");
  const answer = runWorker({
    body,
    environment: { kind: "browser-globals", globals: ["window", "document"] }
  });
  assert.equal(answer.results[0].outcome, "observed");
});

test("history.pushState and replaceState really set history.state, and length follows the spec", () => {
  // The shape of the bug this rules out: `replaceState` used to be a no-op, so
  // a module that called it and then read `history.state` on the very next
  // line saw the *old* state forever -- `null` if nothing had set it yet.
  // `@solidjs/router`'s `saveCurrentDepth` does exactly that at import time,
  // unconditionally, in every browser-conditioned mode: it replaces state with
  // `{ ..., _depth }` and immediately reads `history.state._depth`, so the old
  // no-op crashed every import with `Cannot read properties of null` -- a
  // defect the shim manufactured, not a fact about the package. The shim is
  // still an approximation (`go`/`back`/`forward` stay inert), so the claim
  // pinned here is narrower than full browser fidelity: state really lands,
  // it is structured-cloned rather than aliased, and `length` moves the way
  // the spec says for the two mutators that are implemented.
  // A "kind" probe only ever reports `typeof` the export, so the fixture
  // asserts each fact itself and throws (making the import fail) if the shim
  // regresses -- the same style `saveCurrentDepth` itself relies on, since it
  // reads `history.state._depth` unchecked on the line right after the call.
  const body = [
    "function assertEqual(actual, expected, label) {",
    "  if (JSON.stringify(actual) !== JSON.stringify(expected)) {",
    "    throw new Error(`${label}: expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`);",
    "  }",
    "}",
    "assertEqual(history.state, null, 'initial state');",
    "history.replaceState({ marker: 'replaced' }, '');",
    "assertEqual(history.state, { marker: 'replaced' }, 'state after replaceState');",
    "assertEqual(history.length, 1, 'length after replaceState');",
    "history.pushState({ marker: 'pushed' }, '');",
    "assertEqual(history.state, { marker: 'pushed' }, 'state after pushState');",
    "assertEqual(history.length, 2, 'length after pushState');",
    "const handed = { marker: 'cloned' };",
    "history.replaceState(handed, '');",
    "if (history.state === handed) throw new Error('state aliases the caller object; a browser structured-clones it');",
    "handed.marker = 'mutated-after-the-fact';",
    "assertEqual(history.state, { marker: 'cloned' }, 'state after caller mutation');",
    "let cloneError = null;",
    "try { history.pushState({ f: () => {} }, ''); } catch (error) { cloneError = error.name; }",
    "assertEqual(cloneError, 'DataCloneError', 'uncloneable state');",
    "export const report = true;"
  ].join("\n");
  const answer = runWorker({
    body,
    environment: { kind: "browser-globals", globals: ["window", "history"] }
  });
  assert.equal(answer.results[0].outcome, "observed", answer.results[0].error);
  assert.equal(answer.results[0].observation.typeofValue, "boolean");
});

test("document.head.append and prepend exist, exactly like a real Element", () => {
  // `append`/`prepend` are the modern, variadic form of `appendChild` and are
  // real methods on every `Element`. `@solidjs/start-devtools`'s development
  // build calls `document.head.append(...)` at import time to mount its own
  // style tag; without this the shim's `document.head` had no such method at
  // all, so the import threw `TypeError: ... .append is not a function` --
  // a shim gap, not an honest fact about the package.
  const body = [
    "if (typeof document.head.append !== 'function') throw new Error('document.head.append is not a function');",
    "if (typeof document.head.prepend !== 'function') throw new Error('document.head.prepend is not a function');",
    "document.head.append(document.createElement('style'), 'text');",
    "document.head.prepend(document.createElement('style'));",
    "export const report = true;"
  ].join("\n");
  const answer = runWorker({
    body,
    environment: { kind: "browser-globals", globals: ["window", "document"] }
  });
  assert.equal(answer.results[0].outcome, "observed", answer.results[0].error);
  assert.equal(answer.results[0].observation.typeofValue, "boolean");
});

test("a global Node already provides is left alone rather than replaced by a fake", () => {
  // `navigator` is real in modern Node. Overwriting it would make the
  // observation weaker than it has to be, so it is reported as present.
  const answer = runWorker({
    body: "export const report = typeof navigator;",
    environment: { kind: "browser-globals", globals: ["navigator", "window"] }
  });
  assert.deepEqual(answer.environment.shimmed, ["window"]);
  assert.deepEqual(answer.environment.present, ["navigator"]);
});

test("an import that still throws with the shim in place is undriven exactly as before", () => {
  // Rule (d): the shim removes one reason an import fails. It does not make a
  // failing import into an observation.
  const answer = runWorker({
    body: "throw new Error('module body refuses');\nexport const report = 1;",
    environment: { kind: "browser-globals", globals: ["window"] }
  });
  assert.equal(answer.results[0].outcome, "import-failed");
  assert.match(answer.results[0].error, /module body refuses/);
  assert.deepEqual(answer.environment.shimmed, ["window"]);
});

test("the probe report records the environment, the accounting and which modes were inert", () => {
  const report = buildProbeReport({
    contract: { package: { name: "p", version: "1.0.0" } },
    contractHash: "sha256:x",
    contractPath: "/tmp/p.json",
    installed: { version: "1.0.0" },
    generator: null,
    probeDriver: "solid-checker@test",
    dialect: "solid-v1",
    runtime: { package: "solid-js", version: "1.9.14" },
    modes: PROBE_MODES,
    discovery: { enabled: true, parameters: [0, 1] },
    environment: {
      client: { kind: "browser-globals", shimmed: ["window", "document"], present: ["navigator"] },
      server: { kind: "none", shimmed: [], present: [] }
    },
    sessions: [
      {
        mode: "client",
        started: 3,
        restarts: 2,
        failed: 0,
        completed: true,
        runtime: { reruns: true }
      },
      { mode: "server", started: 1, restarts: 0, failed: 1, completed: false }
    ],
    claims: [],
    incompleteness: []
  });
  assert.equal(report.environment.shimmedAnyMode, true);
  assert.deepEqual(report.environment.modes.client.shimmed, ["document", "window"]);
  assert.deepEqual(report.environment.modes.server, { kind: "none", shimmed: [], present: [] });
  // The runtime capability the worker measured rides along per mode, so a report
  // can say *which* modes withdrew their callback claims because the runtime was
  // asked and answered that it re-runs nothing. A mode whose processes never got
  // that far records `null` rather than an inert answer nobody measured.
  assert.deepEqual(report.sessions, {
    started: 4,
    restarts: 2,
    failed: 1,
    byMode: {
      client: { started: 3, restarts: 2, failed: 0, completed: true, runtime: { reruns: true } },
      server: { started: 1, restarts: 0, failed: 1, completed: false, runtime: null }
    }
  });
});

test("a report with no shimmed mode says so rather than omitting the block", () => {
  const report = buildProbeReport({
    contract: { package: { name: "p", version: "1.0.0" } },
    contractHash: "sha256:x",
    contractPath: "/tmp/p.json",
    installed: { version: "1.0.0" },
    generator: null,
    probeDriver: "solid-checker@test",
    dialect: "solid-v1",
    runtime: { package: "solid-js", version: "1.9.14" },
    modes: PROBE_MODES,
    discovery: { enabled: true, parameters: [0, 1] },
    environment: { client: { kind: "none", shimmed: [], present: [] } },
    sessions: [],
    claims: [],
    incompleteness: []
  });
  assert.equal(report.environment.shimmedAnyMode, false);
  assert.deepEqual(report.sessions, { started: 0, restarts: 0, failed: 0, byMode: {} });
});

test("an asynchronous throw from package code costs the process, not the mode", () => {
  // A deferred callback the probe planted, or a promise the package left
  // rejected, throws outside every `try` in the worker. Before the guard the
  // process died with status 1 and an empty stdout, so the parent had *no*
  // results for that mode: every probe already answered was discarded, and a
  // whole-process failure names no probe to retry past, so the mode ended
  // there. Two corpus rows lost their verification exactly that way.
  const answer = runWorker({
    // Queued as a microtask rather than a timer so the throw lands while the
    // worker is still awaiting its next probe -- the position a real deferred
    // package callback throws from, and the one this test can make
    // deterministic.
    body: [
      "queueMicrotask(() => { throw new Error('deferred package throw'); });",
      "export const report = 1;"
    ].join("\n"),
    probeCount: 3,
    environment: { kind: "browser-globals", globals: ["window"] }
  });
  // The process still writes a readable report and exits 0 -- `runWorker`
  // asserts the exit status -- instead of dying with status 1 and an empty
  // stdout. Whatever it had answered survives, and `completed: false` tells the
  // parent to restart for the rest.
  assert.equal(answer.completed, false);
  assert.ok(Array.isArray(answer.results));
  assert.match(answer.aborted, /uncaughtException/);
  assert.match(answer.aborted, /deferred package throw/);
});

test("an aborted session names the abort as the reason it could not reach a claim", () => {
  const session = {
    mode: "client",
    conditions: ["browser"],
    probes: [{ id: "p1" }, { id: "p2" }]
  };
  let attempts = 0;
  const { results, accounting } = runSessionWithRestarts({
    session,
    spawn: () => {
      attempts += 1;
      return { completed: false, aborted: "uncaughtException: Error: boom", results: [] };
    }
  });
  // No progress was made, so it does not restart into the same abort.
  assert.equal(attempts, 1);
  assert.equal(accounting.failed, 1);
  assert.deepEqual(
    results.map(result => result.outcome),
    ["session-failed", "session-failed"]
  );
  assert.match(results[0].error, /aborted by package code running outside a probe/);
  assert.match(results[0].error, /boom/);
});

test("a session that aborted after answering something is restarted for the rest", () => {
  const session = {
    mode: "development",
    conditions: ["browser", "development"],
    probes: [{ id: "p1" }, { id: "p2" }, { id: "p3" }]
  };
  const attempts = [];
  const { results, accounting } = runSessionWithRestarts({
    session,
    spawn: probes => {
      attempts.push(probes.map(probe => probe.id));
      return probes.length > 1
        ? {
            completed: false,
            aborted: "uncaughtException: Error: boom",
            results: [{ id: probes[0].id, outcome: "observed" }]
          }
        : { completed: true, results: [{ id: probes[0].id, outcome: "observed" }] };
    }
  });
  assert.deepEqual(attempts, [["p1", "p2", "p3"], ["p2", "p3"], ["p3"]]);
  assert.deepEqual(
    results.map(result => result.outcome),
    ["observed", "observed", "observed"]
  );
  assert.equal(accounting.restarts, 2);
  assert.equal(accounting.completed, true);
});

// ---------------------------------------------------------------------------
// The worker's runtime capability self-check
// ---------------------------------------------------------------------------

/// A reactive runtime in about twenty lines: enough of a graph that a memo
/// re-runs when a signal it read is written, which is the whole property the
/// self-check measures. `wrapMemo` is a genuinely tracked callback parameter.
const REACTIVE_RUNTIME = [
  "let listener = null;",
  "export function createSignal(value) {",
  "  const subscribers = new Set();",
  "  return [",
  "    () => { if (listener) subscribers.add(listener); return value; },",
  "    next => {",
  "      value = next;",
  "      const queued = [...subscribers];",
  "      subscribers.clear();",
  "      for (const run of queued) run();",
  "      return next;",
  "    }",
  "  ];",
  "}",
  "export function createMemo(fn) {",
  "  let value;",
  "  const run = () => {",
  "    const previous = listener;",
  "    listener = run;",
  "    try { value = fn(); } finally { listener = previous; }",
  "  };",
  "  run();",
  "  return () => value;",
  "}",
  "export const createRoot = fn => fn(() => {});",
  "export function untrack(fn) {",
  "  const previous = listener;",
  "  listener = null;",
  "  try { return fn(); } finally { listener = previous; }",
  "}",
  "export const wrapMemo = compute => createMemo(compute);",
  ""
].join("\n");

/// The shape of both audited releases' server builds: a signal is a constant, a
/// memo computes once and caches, and a root runs its body. Nothing can re-run,
/// so nothing about attribution is observable -- and every primitive is still
/// present, which is why a name-based or shape-based check would not notice.
const INERT_RUNTIME = [
  "export const createSignal = value => [() => value, () => value];",
  "export const createMemo = fn => { const value = fn(); return () => value; };",
  "export const createRoot = fn => fn(() => {});",
  "export const untrack = fn => fn();",
  "export const wrapMemo = compute => createMemo(compute);",
  ""
].join("\n");

const callbackProbe = (id, specifier, name) => ({
  id,
  type: "callback",
  specifier,
  export: name,
  parameter: 0,
  arguments: ["probe-callback"]
});

test("the worker asks its runtime whether anything re-runs, and stamps every observation", () => {
  const probes = [callbackProbe("p1", "solid-js", "wrapMemo")];
  const reactive = runWorker({ solid: REACTIVE_RUNTIME, probes, environment: { kind: "none", globals: [] } });
  assert.deepEqual(reactive.runtime, { reruns: true });
  assert.equal(reactive.results[0].outcome, "observed");
  assert.deepEqual(reactive.results[0].runtime, { reruns: true });
  // The control interval is reported alongside the baseline, so the driver can
  // tell a write-caused re-run from a callback that runs on its own.
  const seen = reactive.results[0].observation;
  assert.equal(seen.runsBeforeWrite, 1);
  assert.equal(seen.runsAfterControl, 1);
  assert.equal(seen.runsAfterWrite, 2);
  assert.equal(classifyExecution(seen), "tracked");

  const inert = runWorker({ solid: INERT_RUNTIME, probes, environment: { kind: "none", globals: [] } });
  assert.deepEqual(inert.runtime, { reruns: false });
  assert.deepEqual(inert.results[0].runtime, { reruns: false });
  // And this is the manufactured pass the stamp exists to withdraw: the same
  // definitionally tracked export reads `inline` in the inert runtime, because
  // the only thing the counters can record there is the synchronous call.
  assert.equal(classifyExecution(inert.results[0].observation), "inline");
});

test("a runtime whose primitives throw is reported as re-running nothing, with the throw", () => {
  // The default `solid-js` stub of these tests exports a `createSignal` that
  // returns `[]`, so the self-check destructures undefined and throws. Failing
  // closed there is what keeps a broken runtime from certifying anything.
  const answer = runWorker({
    probes: [callbackProbe("p1", "env-fixture", "wrap")],
    body: "export const wrap = callback => callback();\n",
    environment: { kind: "none", globals: [] }
  });
  assert.equal(answer.runtime.reruns, false);
  assert.match(answer.runtime.error, /TypeError|not a function|undefined/);
});

test("the capability is measured per runtime, so one session reports both answers", () => {
  // The concrete case: probing solid-js@1.9.14 in `server` mode, `.` resolves to
  // the non-reactive `dist/server.js` while `./jsx-dev-runtime` resolves
  // unconditionally to `dist/solid.js` and drives its own probes. Here
  // `plain-fixture` stands for the entrypoint the inert project runtime drives
  // and `self-driving` for the one that carries a reactive runtime of its own --
  // `drivesItself` is true for it because it exports all four primitives.
  const answer = runWorker({
    mode: "server",
    solid: INERT_RUNTIME,
    packages: {
      "plain-fixture": "export const wrap = callback => callback();\n",
      "self-driving": REACTIVE_RUNTIME
    },
    probes: [
      callbackProbe("p1", "plain-fixture", "wrap"),
      callbackProbe("p2", "self-driving", "wrapMemo")
    ],
    environment: { kind: "none", globals: [] }
  });
  const byId = Object.fromEntries(answer.results.map(result => [result.id, result]));
  // The session-level record is the project runtime's, and it is not the answer
  // for every observation in the session.
  assert.deepEqual(answer.runtime, { reruns: false });
  assert.deepEqual(byId.p1.runtime, { reruns: false });
  assert.deepEqual(byId.p2.runtime, { reruns: true });
  // Which is exactly the difference that matters: the inert one's counters name
  // `inline` for a callback whose attribution is unobservable, and the
  // self-driven one's name `tracked` truthfully.
  assert.equal(classifyExecution(byId.p1.observation), "inline");
  assert.equal(classifyExecution(byId.p2.observation), "tracked");
});

test("the fake element carries the members a cleanup path reaches for", () => {
  // `el.remove()` in an `onCleanup` is the shape that cost a row: a primitive
  // that appends a measuring element and removes it on dispose throws where the
  // worker cannot attribute the throw to a probe.
  const answer = runWorker({
    body: [
      "const el = document.createElement('div');",
      "document.body.appendChild(el);",
      "el.remove();",
      "export const report = { matched: el.matches('div'), walker: !!document.createTreeWalker(el) };"
    ].join("\n"),
    environment: { kind: "browser-globals", globals: ["window", "document"] }
  });
  assert.equal(answer.results[0].outcome, "observed");
});
