// One isolated worker-v5 session for a stable-v1 proposal. Recipe modules emit raw
// semantic events; Rust later validates and classifies the complete run.
//
// The worker decides nothing semantic and asserts nothing about its own
// environment. Inside a certification transaction Rust launches this file
// directly from a private directory, having hashed both this image and the
// Node executable against digests compiled into the verifier. Four
// consequences shape the protocol below:
//
//   * Both frames are written to descriptor 3, a pipe the launcher created and
//     handed over, never to stdout. The recipe imports the analyzed package
//     into this realm, so package top-level code can replace
//     `process.stdout.write`; it cannot reach *the harness's own write binding
//     by name*. The descriptor number itself is not a secret — in-realm code
//     can name it (`fs.writeSync(3, …)`), and a native addon or an
//     fd-inheriting child inherits it — so what descriptor 3 buys is that the
//     report path does not travel over a stream package code is expected to
//     hold. A forged frame is refused for a different reason: it would have to
//     carry this launch's nonce and the launched pid, and a third frame on the
//     descriptor is a protocol refusal rather than a choice of which to
//     believe. Rust nulls stdout and requires exactly the startup frame and
//     one run frame on descriptor 3.
//   * Everything the report path needs — the raw descriptor write,
//     `Buffer.from`, the frame serializer, the process identity fields — is
//     captured into module-local bindings while this module evaluates, which
//     is before the recipe (and therefore the package) is imported. Nothing is
//     looked up by name afterwards.
//   * A frame is built as null-prototype records and serialized by
//     `contract-probe-harness.mjs`'s own serializer, because `JSON.stringify`
//     performs `Get(value, "toJSON")` on every object it visits: a package
//     defining `Object.prototype.toJSON` (or `Array.prototype.toJSON`, for the
//     events container) was handed the real frame and could return a laundered
//     one. That module's header carries the full reasoning. As a second,
//     independent answer *to that attack*, the intrinsic prototypes are frozen
//     below before the recipe is imported, so a package that tries the patch
//     throws — which refuses the gate — instead of succeeding. Each half
//     suffices against `toJSON`; neither is a general realm sandbox, and
//     nothing on the report path depends on the freeze holding.
//   * The startup frame is written before stdin is read, so Rust can check the
//     protocol, this launch's nonce, and the observed Node version, platform,
//     and architecture and kill the process before handing it a session.
//
// The reported environment is `session.mode.environment` verbatim, copied
// *before* the recipe runs because the recipe is handed the same parsed
// session. It is transport data that must equal what Rust computed; a digest
// this process computed about itself would be worth nothing, so it computes
// none.

import { createHash, randomUUID } from "node:crypto";
import { readFileSync, writeSync } from "node:fs";
import { createRequire } from "node:module";
import * as moduleRuntime from "node:module";
import { pathToFileURL } from "node:url";

import {
  PROBE_WORKER_PROTOCOL,
  adoptFrameValue,
  appendFrameItem,
  createFrameList,
  createFrameRecord,
  createRuntimeProbeHarness,
  serializeFrame
} from "./contract-probe-harness.mjs";

const PROTOCOL = PROBE_WORKER_PROTOCOL;
const STARTUP_FORMAT = "solid-checker-probe-worker-startup";
const REPORT_DESCRIPTOR = 3;

const serialize = serializeFrame;
const parse = JSON.parse;
const writeDescriptor = writeSync;
const bufferFrom = Buffer.from;
const hash = createHash;
const uuid = randomUUID;
const toFileUrl = pathToFileURL;
const requireFrom = createRequire;
// Captured while this module evaluates, like every other primordial the report
// path needs. `import.meta.resolve` is a per-module closure over this file's
// URL, so it is not a name package code can reach — but it is read here, once,
// for the same reason as the rest.
const resolveModule = import.meta.resolve;
const freeze = Object.freeze;
const processId = process.pid;
const nodeVersion = process.version;
const nodePlatform = process.platform;
const nodeArchitecture = process.arch;
// The launch nonce stays in the environment: it binds this *process* to the
// harness that spawned it, which is true from the moment it boots and is what
// the startup frame below answers with. The recipe module does not — it is
// per *session*, and a worker is allowed to be booted before its session
// exists (the pre-boot pool), so it arrives in the session frame instead.
const nonce = process.env.SOLID_CHECKER_PROBE_NONCE ?? "";
const stdin = process.stdin;
// Captured too: the failure path runs *after* the package was imported, and a
// patched `Error` with its own `Symbol.hasInstance` could throw inside the
// `catch` that is meant to contain the failure.
const ErrorConstructor = Error;
const asString = String;
const readBytes = readFileSync;
// Namespace reads keep ordinary audit workers usable on runtimes without
// these optional APIs. The controlled profile only runs the compiled-pinned
// Node and fails inside the transaction if either capability is unavailable.
const installHooks = moduleRuntime.registerHooks;
const stripTypes = moduleRuntime.stripTypeScriptTypes;
const apply = Reflect.apply;
const hasOwn = Object.hasOwn;
const ownKeys = Object.keys;
// The string methods the failure summary needs, captured as functions so a
// recipe that patched `String.prototype` cannot rewrite what the worker says
// about the failure it caused.
const stringSlice = String.prototype.slice;
const stringIndexOf = String.prototype.indexOf;
const stringEndsWith = String.prototype.endsWith;

// Nothing after this line may add a property to an intrinsic prototype, and
// nothing here needs to. A package top level that tries — the
// `Object.prototype.toJSON` laundering attack above, `Array.prototype.push`,
// `Symbol.iterator` — throws on a frozen prototype under `defineProperty` and
// in strict-mode assignment, which surfaces as a failed run and refuses the
// gate. Sloppy-mode assignment silently does nothing. The frame
// representation does not depend on this holding: it is the second of two
// independent answers to the `toJSON` attack.
//
// This is a refusal direction and never a pass: a *benign* package whose top
// level does `Object.prototype.toString = fn` in strict mode throws here and
// refuses the gate, so a closure that could have certified does not. That
// trade is recorded in `docs/adr/0006-probe-harness-binding.md`, the fixture
// README, and every recipe's `coverageLimitations`.
//
// `frozen-intrinsics.mjs` in the closed-domain-probe-gate fixture reports
// `Object.isFrozen` for all three from inside a launched worker, so removing
// any of these three lines fails a test rather than quietly widening the realm.
freeze(Object.prototype);
freeze(Array.prototype);
freeze(Function.prototype);

function digest(value) {
  return `sha256:${hash("sha256").update(value).digest("hex")}`;
}

// The bounded first line of what was thrown -- `ReferenceError: document is
// not defined` -- so a mandatory veto that did not complete can say why. The
// digest above stays the outcome's identity in evidence; Rust carries this
// summary no further than the withheld record's reason (ADR 0036).
const SUMMARY_LIMIT = 240;
function summarizeFailure(error) {
  let text;
  if (error instanceof ErrorConstructor) {
    const name = asString(error.name ?? "Error");
    const message = asString(error.message ?? "");
    text = message === "" ? name : `${name}: ${message}`;
  } else {
    text = asString(error);
  }
  const newline = apply(stringIndexOf, text, ["\n"]);
  if (newline !== -1) text = apply(stringSlice, text, [0, newline]);
  if (text.length > SUMMARY_LIMIT) text = `${apply(stringSlice, text, [0, SUMMARY_LIMIT])}\u2026`;
  return text === "" ? "Error" : text;
}

function report(frame) {
  const bytes = bufferFrom(`${serialize(frame)}\n`, "utf8");
  let offset = 0;
  while (offset < bytes.length) {
    offset += writeDescriptor(REPORT_DESCRIPTOR, bytes, offset, bytes.length - offset);
  }
}

const startup = createFrameRecord();
startup.format = STARTUP_FORMAT;
startup.protocol = PROTOCOL;
startup.nonce = nonce;
startup.nodeVersion = nodeVersion;
startup.platform = nodePlatform;
startup.architecture = nodeArchitecture;
report(startup);

let input = "";
for await (const chunk of stdin) input += chunk;
const session = parse(input);
// Both are read before the recipe — and therefore the package — runs, and
// copied into frame records: the recipe holds the parsed `session`, so a read
// taken afterwards would report whatever it left behind.
const sessionId = adoptFrameValue(session.id);
const environment = adoptFrameValue(session.mode.environment);
// Read here for the same reason as those two, and fails closed: a frame that
// names no recipe cannot be run, and must never fall back to an environment
// value a pooled worker would have inherited from an earlier session.
const recipePath = asString(session.recipe ?? "");
if (recipePath === "") {
  throw new ErrorConstructor("probe session frame names no recipe module");
}
// What the plan's own specifier resolves to in this realm, observed *before*
// the recipe — and therefore the package — is imported.
//
// Rust compares this against the single runtime target the Type Facts witness
// read for the selected export conditions. Which file a package's `exports`
// answers with depends on the condition set the interpreter applies, and that
// set is not the one the artifact case was selected under: this Node applies
// `module-sync` and `node-addons` too, and `module-sync` wins over `import`
// in a package that lists it first. So the answer is reported and compared
// rather than assumed.
//
// The CommonJS answer is resolved from the *recipe's* own URL, which is exact.
// The ESM answer is this module's, because `import.meta.resolve` is per-module
// and there is no public API to resolve from another module's URL; the two
// directories carry identical package scopes and share one `node_modules`
// ancestry, and both are watched trees.
//
// A session may also name the *dependency* specifiers its recipe declared it
// needs. Each is resolved the same way and reported in the order asked, and
// Rust requires every answer to name a file inside that dependency's
// authenticated private copy — which is what makes "the package's own
// `import "solid-js"` reached authenticated bytes" a checked property rather
// than an assumption about the layout.
const resolutionRequest = session.resolution;
let resolution = null;
if (resolutionRequest) {
  resolution = createFrameRecord();
  resolution.specifier = asString(resolutionRequest.specifier);
  resolution.importKind = asString(resolutionRequest.importKind);
  const resolveEsm = specifier => {
    try {
      return asString(resolveModule(specifier));
    } catch (error) {
      return `unresolved:${asString(error?.code ?? "error")}`;
    }
  };
  const resolveRequire = specifier => {
    try {
      return asString(requireFrom(toFileUrl(recipePath).href).resolve(specifier));
    } catch (error) {
      return `unresolved:${asString(error?.code ?? "error")}`;
    }
  };
  resolution.esm = resolveEsm(resolution.specifier);
  resolution.require = resolveRequire(resolution.specifier);
  // An index loop over own indices, not `for…of`: the container is whatever
  // the parsed session carried, and a `Symbol.iterator` lookup is a name.
  const requested = resolutionRequest.dependencies;
  if (requested) {
    const dependencies = createFrameList();
    for (let index = 0; index < requested.length; index += 1) {
      const specifier = asString(requested[index]);
      const reported = createFrameRecord();
      reported.specifier = specifier;
      reported.esm = resolveEsm(specifier);
      reported.require = resolveRequire(specifier);
      appendFrameItem(dependencies, reported);
    }
    resolution.dependencies = dependencies;
  }
}
const isolation = createFrameRecord();
isolation.process = `${processId}:${uuid()}`;
isolation.realm = uuid();
isolation.moduleInstance = uuid();
const harness = createRuntimeProbeHarness(session);
// Captured before recipe import. A recipe receives `session`, so no profile
// field is ever read from it after untrusted code has run.
const requestedExecution = session.execution;
const execution = requestedExecution ? createFrameRecord() : null;
let sourceUrl;
let exportName;
let consume = false;
let controlledEdgeKeys;
let resolvedEdgeKeys;
if (execution) {
  execution.binding = adoptFrameValue(requestedExecution);
  execution.loaded = false;
  execution.consumerCompleted = false;
  execution.stage = "requested";
}
// ADR 0039: the `.jsx` modules Rust proved carry no JSX, which this worker
// may execute as ECMAScript. Read from the session *before* any package or
// recipe code runs, and only in the ordinary published-bytes lane -- a
// controlled-execution profile serves its own module set and never resolves a
// `.jsx`.
//
// The hook is deliberately narrow in both directions. It answers only the
// exact file URLs Rust named, each of which it re-reads and re-digests, so a
// `.jsx` swapped under the private tree between Rust's admission and this load
// throws instead of executing. And it *refuses* every other `.jsx` URL by
// name, so a module the checker did not admit cannot reach the interpreter
// through this hook and be reported as a plain load failure.
//
// Node is the independent second answer to the premise itself: every JSX form
// is a syntax error in ECMAScript, so a module admitted in error throws here
// and withholds the candidate rather than running as something else.
const requestedJsxFreeEsm = session.jsxFreeEsm;
if (!execution && Array.isArray(requestedJsxFreeEsm) && requestedJsxFreeEsm.length > 0) {
  const jsxFreeByUrl = { __proto__: null };
  for (const entry of requestedJsxFreeEsm) {
    const record = createFrameRecord();
    record.path = asString(entry.path);
    record.sha256 = asString(entry.sha256);
    jsxFreeByUrl[toFileUrl(record.path).href] = record;
  }
  installHooks({
    load(url, context, next) {
      if (!apply(stringEndsWith, url, [".jsx"])) return next(url, context);
      if (!hasOwn(jsxFreeByUrl, url)) {
        throw new ErrorConstructor(`no jsx-free premise covers ${url}`);
      }
      const admitted = jsxFreeByUrl[url];
      const source = readBytes(admitted.path, "utf8");
      if (digest(source) !== admitted.sha256) {
        throw new ErrorConstructor(`jsx-free module ${url} is not the admitted bytes`);
      }
      if (context.conditions.includes("require")) {
        throw new ErrorConstructor(`jsx-free module ${url} may not be consumed as CommonJS`);
      }
      return { format: "module", source, shortCircuit: true };
    }
  });
}
let outcome;
try {
  if (execution) {
    const binding = execution.binding;
    if (!["node-strip-inert-esm-v1", "node-strip-import-free-esm-v1", "node-strip-relative-ts-graph-esm-v1"].includes(binding.profile) || resolution?.importKind !== "esm") {
      throw new ErrorConstructor("unsupported controlled execution profile or import kind");
    }
    sourceUrl = toFileUrl(binding.sourcePath).href;
    exportName = binding.exportName;
    consume = binding.consumer;
    if (sourceUrl !== resolution.esm) throw new ErrorConstructor("profile source resolution mismatch");
    const isGraph = binding.profile === "node-strip-relative-ts-graph-esm-v1";
    if (isGraph !== (binding.modules !== undefined && binding.modules.length > 0)) {
      throw new ErrorConstructor("profile module graph mismatch");
    }
    if (!isGraph && ((binding.modules?.length ?? 0) !== 0 || (binding.edges?.length ?? 0) !== 0)) {
      throw new ErrorConstructor("single-module profile carried graph fields");
    }
    const requestedModules = isGraph ? binding.modules : [binding];
    const controlledModules = createFrameRecord();
    for (let moduleIndex = 0; moduleIndex < requestedModules.length; moduleIndex += 1) {
      const module = requestedModules[moduleIndex];
      const moduleUrl = toFileUrl(module.sourcePath).href;
      if (hasOwn(controlledModules, moduleUrl)) throw new ErrorConstructor("duplicate controlled source URL");
      const source = readBytes(module.sourcePath, "utf8");
      const derived = readBytes(module.derivedPath, "utf8");
      const output = stripTypes(source, { mode: "strip" });
      if (digest(source) !== module.sourceDigest || digest(output) !== module.outputDigest || derived !== output) {
        throw new ErrorConstructor("profile transformer/input/output mismatch");
      }
      const verified = createFrameRecord();
      verified.output = output;
      controlledModules[moduleUrl] = verified;
    }
    const rootModule = controlledModules[sourceUrl];
    if (!rootModule || digest(readBytes(binding.sourcePath, "utf8")) !== binding.sourceDigest || digest(rootModule.output) !== binding.outputDigest) {
      throw new ErrorConstructor("profile root module binding mismatch");
    }
    controlledEdgeKeys = createFrameRecord();
    resolvedEdgeKeys = createFrameRecord();
    const requestedEdges = binding.edges ?? [];
    for (let edgeIndex = 0; edgeIndex < requestedEdges.length; edgeIndex += 1) {
      const edge = requestedEdges[edgeIndex];
      const importerUrl = toFileUrl(edge.importerPath).href;
      const targetUrl = toFileUrl(edge.targetPath).href;
      if (!hasOwn(controlledModules, importerUrl) || !hasOwn(controlledModules, targetUrl)) {
        throw new ErrorConstructor("profile edge names an unbound module");
      }
      const key = `${importerUrl}\0${edge.specifier}`;
      if (hasOwn(controlledEdgeKeys, key)) throw new ErrorConstructor("duplicate profile edge key");
      controlledEdgeKeys[key] = targetUrl;
    }
    execution.stage = "transform-verified";
    // Only this exact source URL receives a format override. Node still owns
    // package resolution; Rust compares its answer with the selected snapshot.
    // No suffix inference, aliases, imports from the subject, or CJS fallback.
    installHooks({
      resolve(specifier, context, nextResolve) {
        if (hasOwn(controlledModules, context.parentURL)) {
          const key = `${context.parentURL}\0${specifier}`;
          if (!hasOwn(controlledEdgeKeys, key)) throw new ErrorConstructor("controlled profile module import refused");
          if (context.conditions.includes("require")) throw new ErrorConstructor("profile CommonJS consumption refused");
          resolvedEdgeKeys[key] = true;
          return { url: controlledEdgeKeys[key], shortCircuit: true };
        }
        const result = nextResolve(specifier, context);
        if (ownKeys(controlledModules).some((url) => result.url.startsWith(`${url}?`) || result.url.startsWith(`${url}#`))) {
          throw new ErrorConstructor("profile source URL variant refused");
        }
        if (hasOwn(controlledModules, result.url) && context.conditions.includes("require")) {
          throw new ErrorConstructor("profile CommonJS consumption refused");
        }
        return result;
      },
      load(url, context, nextLoad) {
        if (!hasOwn(controlledModules, url)) return nextLoad(url, context);
        if (url === sourceUrl) {
          execution.loaded = true;
          execution.stage = "module-loaded";
        }
        return { format: "module", source: controlledModules[url].output, shortCircuit: true };
      }
    });
  }
  outcome = createFrameRecord();
  if (consume && execution.binding.profile === "node-strip-inert-esm-v1") {
    const subject = await import(sourceUrl);
    harness.emit({ marker: "call", kind: "call", phase: "enter" });
    const result = apply(subject[exportName], undefined, []);
    if (result !== undefined) throw new ErrorConstructor("controlled inert call returned a value");
    execution.consumerCompleted = true;
    execution.stage = "consumer-completed";
    harness.emit({ marker: "call", kind: "call", phase: "exit" });
    await harness.drain(undefined);
    outcome.kind = "completed";
    outcome.events = harness.events();
  } else {
    const module = await import(`${toFileUrl(recipePath).href}?${isolation.moduleInstance}`);
    if (typeof module.runProbeSession !== "function") {
      outcome.kind = "refused";
      outcome.reason = "recipe module exports no runProbeSession function";
    } else {
      // Passed through as it came back, deliberately not `?? {}`: an object
      // literal has `Object.prototype` on its chain, and `drain` then had to
      // decide "did the recipe supply a flush control?" with a lookup that
      // reaches it. `drain` asks for an own property of whatever this is,
      // including `undefined`.
      const controls = await module.runProbeSession(session, harness);
      await harness.drain(controls);
      outcome.kind = "completed";
      outcome.events = harness.events();
      if (consume) {
        if (ownKeys(controlledEdgeKeys ?? {}).some((key) => !hasOwn(resolvedEdgeKeys, key))) {
          throw new ErrorConstructor("controlled profile edge was not resolved");
        }
        execution.consumerCompleted = true;
        execution.stage = "consumer-completed";
      }
    }
  }
} catch (error) {
  outcome = createFrameRecord();
  outcome.kind = "error";
  outcome.details = digest(
    error instanceof ErrorConstructor ? (error.stack ?? error.message) : asString(error)
  );
  outcome.summary = summarizeFailure(error);
}
const run = createFrameRecord();
run.session = sessionId;
run.environment = environment;
run.isolation = isolation;
if (resolution) run.resolution = resolution;
if (execution) run.execution = execution;
run.drainedMicrotasks = harness.drainedMicrotasks();
run.drainedMacrotasks = harness.drainedMacrotasks();
run.outcome = outcome;
report(run);
