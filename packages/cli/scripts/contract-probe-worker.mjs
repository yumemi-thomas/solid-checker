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

/// The marker every faked value carries, and the record of the whole shim.
///
/// Nothing in this file classifies, and the shim does not either -- but a
/// future classification might need to know that the DOM a package saw was
/// fake, and a fake that is indistinguishable from the real thing takes that
/// option away. So every shimmed value is stamped, and `globalThis` carries an
/// inert record a probe body can read:
///
///     globalThis[SHIM_RECORD].shimmed   // the names this process faked
///     window[SHIM_MARKER] === true      // this window is not a browser's
///
/// Both are accessor properties, non-enumerable, so a package's own feature
/// detection (`for...in`, `Object.keys`, `"window" in globalThis`) sees exactly
/// what it would see in a browser and nothing extra.
const SHIM_MARKER = "__solidCheckerProbeShim";
const SHIM_RECORD = "__solidCheckerProbeEnvironment";

/// A value that answers like the browser object it stands for, and admits what
/// it is when asked.
function shimValue(members) {
  const value = { ...members };
  Object.defineProperty(value, SHIM_MARKER, {
    get: () => true,
    enumerable: false,
    configurable: true
  });
  return value;
}

const noop = () => undefined;

/// A minimal `EventTarget` surface. Import-time code that registers a listener
/// is extremely common and throwing there kills the session for the whole
/// entrypoint; registering nothing is inert and observably so.
const eventTarget = () => ({
  addEventListener: noop,
  removeEventListener: noop,
  dispatchEvent: () => true
});

/// The value each shimmable global takes. Built lazily and only for the names
/// the session asked for, so a name absent from `BROWSER_SHIM_GLOBALS` is
/// absent from the process too.
function shimFactories() {
  const style = () =>
    shimValue({
      cssText: "",
      getPropertyValue: () => "",
      getPropertyPriority: () => "",
      setProperty: noop,
      removeProperty: noop
    });
  // `ownerDocument` and `defaultView` are back-references, and a node has to
  // carry them: a package that reaches `node.ownerDocument.addEventListener`
  // in a *deferred* callback throws in a timer, which is an uncaught exception
  // that kills the whole worker process rather than one probe -- taking every
  // remaining claim of that mode, including the `kind` observations
  // verification cannot convert, with it. Both are late-bound getters because
  // the document and the window are built after the first node is.
  let documentValue;
  let windowValue;
  const element = () => {
    const node = shimValue({
      ...eventTarget(),
      style: style(),
      dataset: {},
      // The structural companions of the members above. Each is either
      // something the corpus's packages actually reached for or its immediate
      // neighbour: a node that has `firstChild` and no `childNodes` is a node
      // that throws one line later.
      nodeType: 1,
      tagName: "DIV",
      isConnected: false,
      parentNode: null,
      parentElement: null,
      firstChild: null,
      lastChild: null,
      nextSibling: null,
      previousSibling: null,
      childNodes: [],
      children: [],
      classList: { add: noop, remove: noop, toggle: () => false, contains: () => false },
      contains: () => false,
      appendChild: value => value,
      removeChild: value => value,
      insertBefore: value => value,
      setAttribute: noop,
      getAttribute: () => null,
      removeAttribute: noop,
      hasAttribute: () => false,
      focus: noop,
      blur: noop,
      click: noop,
      // `remove()` is the one that cost a row: a primitive that appends a
      // measuring element and removes it in `onCleanup` throws during dispose,
      // and the throw lands where the worker cannot attribute it.
      remove: noop,
      matches: () => false,
      getBoundingClientRect: () => ({
        x: 0,
        y: 0,
        top: 0,
        left: 0,
        right: 0,
        bottom: 0,
        width: 0,
        height: 0
      }),
      querySelector: () => null,
      querySelectorAll: () => []
    });
    Object.defineProperty(node, "ownerDocument", {
      get: () => documentValue,
      enumerable: true,
      configurable: true
    });
    return node;
  };
  documentValue = shimValue({
    ...eventTarget(),
    // `readyState` is the single most-consulted import-time guard after
    // `typeof window`: a module that defers work until the document is parsed
    // reads it, and "complete" is the state in which such a module registers
    // nothing further.
    readyState: "complete",
    visibilityState: "visible",
    hidden: false,
    title: "",
    documentElement: element(),
    body: element(),
    head: element(),
    createElement: () => element(),
    createTextNode: () => shimValue({ data: "" }),
    createDocumentFragment: () => element(),
    querySelector: () => null,
    querySelectorAll: () => [],
    getElementById: () => null,
    getElementsByTagName: () => [],
    createTreeWalker: () => shimValue({ nextNode: () => null, currentNode: null }),
    activeElement: null
  });
  Object.defineProperty(documentValue, "defaultView", {
    get: () => windowValue ?? null,
    enumerable: true,
    configurable: true
  });
  const locationValue = shimValue({
    href: "http://localhost/",
    protocol: "http:",
    host: "localhost",
    hostname: "localhost",
    port: "",
    pathname: "/",
    search: "",
    hash: "",
    origin: "http://localhost",
    toString: () => "http://localhost/"
  });
  const observer = () => shimValue({ observe: noop, unobserve: noop, disconnect: noop, takeRecords: () => [] });
  const observerClass = () =>
    class {
      constructor() {
        return observer();
      }
    };
  return {
    document: () => documentValue,
    // The window the document points back at. `installEnvironment` calls this
    // once it has built the window object, which is the only way a
    // `node.ownerDocument.defaultView` chain resolves.
    __bindWindow: value => {
      windowValue = value;
    },
    // `navigator.userAgent` is the other guard packages read at import time. It
    // deliberately names this checker rather than impersonating a browser: a
    // package that branches on the UA string should branch on something it can
    // recognise as not-a-browser if it ever wants to.
    navigator: () =>
      shimValue({
        userAgent: "solid-checker-contract-probe",
        platform: "",
        language: "en",
        languages: ["en"],
        maxTouchPoints: 0,
        onLine: true,
        clipboard: shimValue({ readText: async () => "", writeText: async () => undefined })
      }),
    location: () => locationValue,
    screen: () => shimValue({ width: 0, height: 0, availWidth: 0, availHeight: 0 }),
    history: () =>
      shimValue({ length: 0, state: null, pushState: noop, replaceState: noop, back: noop, forward: noop, go: noop }),
    localStorage: () =>
      shimValue({ length: 0, getItem: () => null, setItem: noop, removeItem: noop, clear: noop, key: () => null }),
    sessionStorage: () =>
      shimValue({ length: 0, getItem: () => null, setItem: noop, removeItem: noop, clear: noop, key: () => null }),
    matchMedia: () => () => shimValue({ ...eventTarget(), matches: false, media: "", onchange: null }),
    // Timers, not frames: nothing here paints, and a callback that never runs
    // would silently change what a probe observes about scheduling.
    requestAnimationFrame: () => callback => setTimeout(() => callback(Date.now()), 0),
    cancelAnimationFrame: () => handle => clearTimeout(handle),
    getComputedStyle: () => () => style(),
    MutationObserver: observerClass,
    ResizeObserver: observerClass,
    IntersectionObserver: observerClass
  };
}

/// Defines the requested globals, and reports exactly what it did.
///
/// A name that already exists is never replaced -- `globalThis.navigator` and
/// `globalThis.localStorage` are real in modern Node, and overwriting them with
/// a fake would make the observation weaker than it has to be. Those are
/// reported under `present` so the record distinguishes "Node already had it"
/// from "this process invented it".
function installEnvironment(environment) {
  const record = { kind: environment?.kind ?? "none", shimmed: [], present: [] };
  if (record.kind !== "browser-globals") return record;
  const factories = shimFactories();
  const values = new Map();
  for (const name of environment.globals ?? []) {
    if (!Object.hasOwn(factories, name) && name !== "window" && name !== "self") continue;
    if (name in globalThis) {
      record.present.push(name);
      continue;
    }
    record.shimmed.push(name);
    values.set(name, undefined);
  }
  // `window` and `self` are the whole point of the exercise and are built last,
  // from whatever the loop above settled, so `window.document` is the same
  // object as the bare `document` a module might read instead.
  const resolve = name => {
    if (!values.has(name)) return name in globalThis ? globalThis[name] : undefined;
    if (values.get(name) === undefined) values.set(name, factories[name]());
    return values.get(name);
  };
  for (const name of record.shimmed) {
    if (name === "window" || name === "self") continue;
    resolve(name);
  }
  let windowValue;
  const buildWindow = () => {
    if (windowValue) return windowValue;
    windowValue = shimValue({
      ...eventTarget(),
      document: resolve("document"),
      navigator: resolve("navigator"),
      location: resolve("location"),
      screen: resolve("screen"),
      history: resolve("history"),
      localStorage: resolve("localStorage"),
      sessionStorage: resolve("sessionStorage"),
      matchMedia: resolve("matchMedia"),
      requestAnimationFrame: resolve("requestAnimationFrame"),
      cancelAnimationFrame: resolve("cancelAnimationFrame"),
      getComputedStyle: resolve("getComputedStyle"),
      MutationObserver: resolve("MutationObserver"),
      ResizeObserver: resolve("ResizeObserver"),
      IntersectionObserver: resolve("IntersectionObserver"),
      innerWidth: 0,
      innerHeight: 0,
      scrollX: 0,
      scrollY: 0,
      devicePixelRatio: 1,
      setTimeout: globalThis.setTimeout,
      clearTimeout: globalThis.clearTimeout,
      setInterval: globalThis.setInterval,
      clearInterval: globalThis.clearInterval
    });
    windowValue.window = windowValue;
    windowValue.self = windowValue;
    windowValue.top = windowValue;
    windowValue.parent = windowValue;
    windowValue.globalThis = globalThis;
    // Closes the loop the DOM has and a flat object does not:
    // `node.ownerDocument.defaultView === window`. A package that walks it and
    // finds `undefined` throws, and when it does so from a timer the throw is
    // uncaught and takes the whole worker with it.
    factories.__bindWindow(windowValue);
    return windowValue;
  };
  for (const name of record.shimmed) {
    const value = name === "window" || name === "self" ? buildWindow() : resolve(name);
    Object.defineProperty(globalThis, name, {
      get: () => value,
      enumerable: false,
      configurable: true
    });
  }
  const frozen = Object.freeze({
    kind: record.kind,
    shimmed: [...record.shimmed],
    present: [...record.present]
  });
  Object.defineProperty(globalThis, SHIM_RECORD, {
    get: () => frozen,
    enumerable: false,
    configurable: true
  });
  return record;
}

// Installed before anything is imported, because the whole failure it exists
// for is a *module-evaluation-time* read of `window`.
const environment = installEnvironment(request.environment);

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
/// The one response this process writes, and the only exit path.
///
/// It lives outside `main` because an *asynchronous* throw from package code --
/// a deferred callback the probe planted, a rejected promise the package left
/// behind -- arrives outside every `try` in this file. Before this the process
/// simply died with status 1 and an empty stdout, so the parent had no results
/// at all for that mode: every probe it had already answered was discarded and
/// the remaining ones were recorded `session-failed` with no restart, because a
/// whole-process failure names no probe to retry past.
///
/// Answering with what was observed and `completed: false` keeps the existing
/// semantics exactly -- the runtime may well be halted, so this process stops --
/// while letting the parent restart for the remainder, which is what it already
/// does after a synchronous throw. The abort reason travels so it can be
/// reported rather than inferred, and it is never attributed to a claim: nothing
/// says which probe scheduled the work that threw.
const results = [];
let responded = false;
function respond({ completed, aborted }) {
  if (responded) return;
  responded = true;
  writeSync(
    1,
    JSON.stringify({
      mode: request.mode,
      dialect: request.dialect,
      // Answered on every session, including the ones that shimmed nothing, so
      // a reader never has to infer "no shim" from a missing field.
      environment,
      completed,
      ...(aborted ? { aborted } : {}),
      results
    })
  );
  process.exit(0);
}

for (const event of ["uncaughtException", "unhandledRejection"]) {
  process.on(event, error =>
    respond({ completed: false, aborted: `${event}: ${String(error?.stack ?? error)}`.slice(0, 2000) })
  );
}

async function main() {
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

  respond({ completed: !halted });
}

await main();
