// The child process `solid-checker contract probe` runs one condition mode in.
//
// It is the only part of the command that imports the package under contract,
// so it is also the only part that executes third-party code. It is copied into
// a temporary directory inside the project's `node_modules` before it runs --
// the same trick `scripts/check-bundled-contracts.mjs` uses for the bundled
// workers -- because that is what makes a bare `import "solid-js"` and a bare
// `import "<package>"` resolve to the releases the *project* installed rather
// than to anything this checker ships.
//
// It classifies nothing. Every body here returns raw counters, and
// `contract-probe-driver.mjs` in the parent decides what they mean, so the
// judgements are unit-testable without an install and a package cannot make the
// verdict by making the observation.
//
// Protocol: argv[2] is a JSON request file; the response is one JSON document on
// stdout. The write is synchronous and the exit immediate because probing leaves
// timers and pending work behind and the parent waits on the process, not the
// stream.

import { readFileSync, writeSync } from "node:fs";

const request = JSON.parse(readFileSync(process.argv[2], "utf8"));

/// The reactive primitives one probe body is driven with.
///
/// 2.0 settles with `flush()`; 1.x has no such function and settles by yielding
/// to a macrotask, exactly as `scripts/contract-probes-solid-v1-core.mjs`
/// records. A write is detached from the probe's own owner in 2.0 because a
/// development build rejects a write made from a parent-owned test root, and a
/// probe stands for an external update.
function buildRuntime(solid) {
  const settle =
    typeof solid.flush === "function"
      ? async () => {
          solid.flush();
          await new Promise(resolve => setTimeout(resolve, 0));
          solid.flush();
        }
      : () => new Promise(resolve => setTimeout(resolve, 0));
  const write =
    typeof solid.flush === "function" && typeof solid.runWithOwner === "function"
      ? (setter, value) => solid.runWithOwner(null, () => setter(value))
      : (setter, value) => setter(value);
  return {
    createSignal: solid.createSignal,
    createMemo: solid.createMemo,
    untrack: solid.untrack,
    settle,
    write,
    async root(body) {
      let dispose = () => {};
      try {
        return await solid.createRoot(async disposer => {
          dispose = disposer;
          return await body();
        });
      } finally {
        dispose();
      }
    }
  };
}

const REACTIVE_PRIMITIVES = ["createSignal", "createMemo", "createRoot", "untrack"];

/// Whether a namespace can drive its own probes.
///
/// This is the generic form of the discipline
/// `scripts/contract-probes-solid-v1-core.mjs` records by hand: an entrypoint
/// that *is* a reactive runtime must be probed with its own primitives. Solid
/// 1.x resolves `.` to `dist/dev.js` in development while `./jsx-runtime` stays
/// on `dist/solid.js`, so a signal made by one and a memo created by the other
/// belong to different schedulers and nothing tracks anything. For an ordinary
/// package the check is false and the project's own `solid-js` drives, which is
/// the same instance the package itself resolved.
function drivesItself(namespace) {
  return REACTIVE_PRIMITIVES.every(name => typeof namespace?.[name] === "function");
}

/// The synthesis vocabulary, resolved to values. `contract-probe-driver.mjs`
/// decides which descriptor each slot gets; this only builds them.
function buildArguments(descriptors, probeCallback) {
  return (descriptors ?? []).map(descriptor => {
    if (descriptor === "probe-callback") return probeCallback;
    if (descriptor === "noop-callback") return () => undefined;
    if (descriptor === "empty-object") return {};
    return undefined;
  });
}

function describeValue(value) {
  if (value === null) return "null";
  if (Array.isArray(value)) return "array";
  return typeof value;
}

/// Runs the export with a probe callback in one slot and reports who owned the
/// reads.
///
/// The export call sits inside a memo so the *call site* has a subscription of
/// its own; the probe callback reads a signal and the signal is then written.
/// Which of the two re-ran is the whole observation, and it is what makes
/// `inline`, `deferred` and `tracked` distinguishable without knowing anything
/// about the export.
async function callbackObservation(runtime, target, probe) {
  const [source, setSource] = runtime.createSignal(0);
  let runs = 0;
  let siteRuns = 0;
  // How many times the export itself was invoked. The call-site memo caches, so
  // this is 1 for a claim whose write does not re-run the site and 2 when it
  // does -- which is a measurement of the export, not a property of the probe
  // type, and is what `evidence.calls` records.
  let targetCalls = 0;
  let ranDuringCall = false;
  let inCall = false;
  const probeCallback = () => {
    runs += 1;
    if (inCall) ranDuringCall = true;
    return source();
  };
  const args = buildArguments(probe.arguments, probeCallback);
  let forcedByAccessorRead = false;
  const site = runtime.createMemo(() => {
    siteRuns += 1;
    inCall = true;
    try {
      targetCalls += 1;
      const result = target(...args);
      // Reading a returned accessor is contract-led: the plan sets
      // `callAccessor` only where the contract itself states `returns.kind`
      // is an accessor, because a lazily computed export never runs its
      // callback until that accessor is read.
      //
      // The read is untracked, and that is the difference between measuring
      // attribution and measuring nothing. A tracked export returns a
      // computation of its own; reading it inside the call-site memo
      // subscribes the *site* to that computation, so the site re-runs on the
      // write and every tracked claim reads as inline. Untracking the read
      // forces the callback to run without lending the site a subscription it
      // did not earn, which leaves the callback's own reads as the only thing
      // the counters can be about.
      if (!probe.callAccessor || typeof result !== "function") return result;
      // Read the returned accessor inside a memo of its own, created inside
      // `untrack`.
      //
      // The read has to happen: a lazily computed export never runs its
      // callback until something reads what it returned, and the plan sets
      // `callAccessor` only where the contract itself states `returns.kind` is
      // an accessor, so the read is contract-led rather than a guess.
      //
      // Where it happens is the whole measurement. Reading it in the call-site
      // memo subscribes the *site* to the export's own computation, so the site
      // re-runs on the write and every tracked claim reads as inline. Reading
      // it under a bare `untrack` fixes that but breaks the other half: an
      // export whose "accessor" is a plain tracked function rather than a memo
      // -- 1.x `mapArray` is exactly this -- has no computation of its own, so
      // an untracked read leaves its reads attributed to nothing and a tracked
      // claim reads as inline again. A fresh memo inside `untrack` is what
      // satisfies both: the reads get a computation to land on, and it is not
      // the site's.
      forcedByAccessorRead = true;
      return runtime.untrack(() => {
        const inner = runtime.createMemo(() => result());
        return inner();
      });
    } finally {
      inCall = false;
    }
  });
  site();
  await runtime.settle();
  site();
  const runsBeforeWrite = runs;
  const siteRunsBeforeWrite = siteRuns;
  runtime.write(setSource, 1);
  await runtime.settle();
  site();
  return {
    ranDuringCall,
    forcedByAccessorRead,
    runsBeforeWrite,
    runsAfterWrite: runs,
    siteRunsBeforeWrite,
    siteRunsAfterWrite: siteRuns,
    calls: targetCalls
  };
}

/// Runs the export once and reports the two properties that separate a reactive
/// accessor from a function that merely returns one shape or another.
///
/// **Reactivity.** A signal read is planted inside a callback the contract
/// states, the returned value is read inside a memo, the signal is written, and
/// the memo is read again. `typeof value === "function"` alone would confirm
/// the claim for any function-returning export, which is a sighting and not an
/// observation of reactivity, so the write is required.
///
/// **Caching.** Reactivity alone is not enough either, and this is the part the
/// first version got wrong. Because the signal read is planted *inside the
/// claimed callback*, a plain forwarding closure -- `(cb) => () => cb()` --
/// re-reads the signal on every read of the returned value, re-runs the outer
/// memo on the write, and passes a reactivity-only test transitively. So the
/// body reads the returned value twice inside one evaluation of the outer memo
/// and reports how many times the planted callback ran across those two reads.
/// A memo accessor recomputes at most once per tracked evaluation; a forwarding
/// closure runs the callback once per read.
///
/// It classifies neither: `contract-probe-driver.mjs` decides what the counters
/// mean, as everywhere else in this file.
async function returnsObservation(runtime, target, probe) {
  const [source, setSource] = runtime.createSignal(0);
  let plantedRuns = 0;
  const planted = () => {
    plantedRuns += 1;
    return source();
  };
  const returned = target(...buildArguments(probe.arguments, planted));
  if (typeof returned !== "function") {
    return { typeofValue: describeValue(returned), reactive: false, calls: 1 };
  }
  const trackedReadCalls = 2;
  let reads = 0;
  let plantedRunsWithinOneRead;
  const outer = runtime.createMemo(() => {
    reads += 1;
    const before = plantedRuns;
    let value;
    for (let read = 0; read < trackedReadCalls; read += 1) value = returned();
    if (plantedRunsWithinOneRead === undefined) plantedRunsWithinOneRead = plantedRuns - before;
    return value;
  });
  outer();
  await runtime.settle();
  outer();
  const before = reads;
  runtime.write(setSource, 1);
  await runtime.settle();
  outer();
  return {
    typeofValue: "function",
    reactive: reads > before,
    trackedReadCalls,
    plantedRunsWithinOneRead: plantedRunsWithinOneRead ?? 0,
    calls: 1
  };
}

/// Runs the requested probes until one of them throws, then stops.
///
/// Stopping is a correctness requirement, not an optimization. Solid 2.0's
/// development build **halts the reactive system permanently** on an uncaught
/// error -- "No further updates will be processed" -- so every probe after a
/// throw observes a runtime where nothing ever re-runs. A tracked callback then
/// looks like an inline one, and the driver would report a false conformance
/// failure against a claim the package honours. The parent restarts a fresh
/// process for whatever is left, which is the only way to un-halt a runtime.
///
/// A failed import is treated the same way: evaluating a module runs package
/// code, and that code can halt the runtime just as a call can. Every probe of
/// that specifier is answered before stopping, so one broken entrypoint costs
/// one restart rather than one per probe.
async function main() {
  const results = [];
  const answered = new Set();
  let halted = false;
  let projectRuntime;
  let runtimeError;
  try {
    projectRuntime = buildRuntime(await import("solid-js"));
  } catch (error) {
    runtimeError = String(error);
  }
  const namespaces = new Map();
  const importNamespace = async specifier => {
    if (!namespaces.has(specifier)) {
      try {
        namespaces.set(specifier, { namespace: await import(specifier) });
      } catch (error) {
        namespaces.set(specifier, { error: String(error) });
      }
    }
    return namespaces.get(specifier);
  };
  const record = result => {
    answered.add(result.id);
    results.push(result);
  };
  // `calls` starts at 0 and only a body that invoked the export raises it. It
  // used to be a per-probe-type constant, which recorded two calls for a
  // `deferred` observation that made one and stamped a call count onto probes
  // -- an import failure, a `typeof` reading -- that invoked nothing at all.
  const describe = probe => ({
    id: probe.id,
    specifier: probe.specifier,
    export: probe.export,
    calls: 0
  });

  for (const probe of request.probes) {
    if (halted) break;
    const base = describe(probe);
    if (runtimeError) {
      record({ ...base, outcome: "threw", error: `no probe runtime: ${runtimeError}` });
      continue;
    }
    const resolved = await importNamespace(probe.specifier);
    if (resolved.error) {
      for (const other of request.probes) {
        if (other.specifier === probe.specifier && !answered.has(other.id)) {
          record({ ...describe(other), outcome: "import-failed", error: resolved.error });
        }
      }
      halted = true;
      break;
    }
    if (!(probe.export in resolved.namespace)) {
      record({ ...base, outcome: "export-missing" });
      continue;
    }
    const value = resolved.namespace[probe.export];
    if (probe.type === "kind") {
      record({ ...base, outcome: "observed", observation: { typeofValue: typeof value } });
      continue;
    }
    if (typeof value !== "function") {
      record({ ...base, outcome: "not-callable" });
      continue;
    }
    const runtime = drivesItself(resolved.namespace)
      ? (resolved.runtime ??= buildRuntime(resolved.namespace))
      : projectRuntime;
    try {
      const observation = await runtime.root(() =>
        probe.type === "returns-accessor"
          ? returnsObservation(runtime, value, probe)
          : callbackObservation(runtime, value, probe)
      );
      record({ ...base, outcome: "observed", observation, calls: observation.calls ?? 0 });
    } catch (error) {
      record({ ...base, outcome: "threw", error: String(error) });
      halted = true;
    }
  }

  writeSync(
    1,
    JSON.stringify({
      mode: request.mode,
      dialect: request.dialect,
      completed: !halted,
      results
    })
  );
  process.exit(0);
}

await main();
