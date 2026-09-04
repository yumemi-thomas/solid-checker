// One isolated runtime-probe-v2 session for a stable-v1 proposal. Recipe modules emit raw
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
import { writeSync } from "node:fs";
import { createRequire } from "node:module";
import { pathToFileURL } from "node:url";

import {
  adoptFrameValue,
  appendFrameItem,
  createFrameList,
  createFrameRecord,
  createRuntimeProbeHarness,
  serializeFrame
} from "./contract-probe-harness.mjs";

const PROTOCOL = "solid-checker-runtime-probe-v2";
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
const recipePath = process.env.SOLID_CHECKER_PROBE_RECIPE;
const nonce = process.env.SOLID_CHECKER_PROBE_NONCE ?? "";
const stdin = process.stdin;
// Captured too: the failure path runs *after* the package was imported, and a
// patched `Error` with its own `Symbol.hasInstance` could throw inside the
// `catch` that is meant to contain the failure.
const ErrorConstructor = Error;
const asString = String;

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
let outcome;
try {
  const module = await import(`${toFileUrl(recipePath).href}?${isolation.moduleInstance}`);
  outcome = createFrameRecord();
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
  }
} catch (error) {
  outcome = createFrameRecord();
  outcome.kind = "error";
  outcome.details = digest(
    error instanceof ErrorConstructor ? (error.stack ?? error.message) : asString(error)
  );
}
const run = createFrameRecord();
run.session = sessionId;
run.environment = environment;
run.isolation = isolation;
if (resolution) run.resolution = resolution;
run.drainedMicrotasks = harness.drainedMicrotasks();
run.drainedMacrotasks = harness.drainedMacrotasks();
run.outcome = outcome;
report(run);
