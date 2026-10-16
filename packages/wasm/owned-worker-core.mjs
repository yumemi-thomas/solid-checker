// The owned check's worker loop (P5 S2, S3b, S3c): both reactors on one
// filesystem -- the in-memory volume of a validated mount, or the directory
// store read on demand from the host -- one check per message, and the
// transport's ledger checked after every check. Every import either reactor
// declares goes through the owned boundary (owned-boundary.mjs), and each
// reactor's whole import list is checked against what this worker serves it
// before it is instantiated. Only the checker gets `solid_owned_host`, the
// project projection (owned-projection.mjs).
//
// The shipped worker (`owned-worker.mjs`) calls `serve(port)` with no
// adapters, and the client posts only `init` and `check` messages, so nothing
// a caller passes to `createOwnedChecker` or `check` reaches `adapters`. The
// adapters exist for the repository's parity gate, which wraps the producer's
// exports or these imports to inject faults as negative controls.
// The pinned @tybys/wasm-util WASI runtime, vendored with physical path
// resolution (vendor/wasm-util/NOTICE). Only this worker uses the copy.
import { WASI } from "./vendor/wasm-util/lib/mjs/wasi/index.mjs";
// The same module instances the runtime's `handleError` and physical resolver
// use: the directory store answers ELOOP as a `WasiError` and shares the
// resolver's substitution bound.
import { WasiError } from "./vendor/wasm-util/lib/mjs/wasi/error.mjs";
import { MAX_SUBSTITUTIONS } from "./vendor/wasm-util/lib/mjs/wasi/physical.mjs";
import { WasiErrno } from "./vendor/wasm-util/lib/mjs/wasi/types.mjs";
import { memfs } from "@napi-rs/wasm-runtime/fs";
import { boundaryImports, coveredImports, protocolImports, u32 } from "./owned-boundary.mjs";
import { createTransport } from "./owned-host.mjs";
import { materializeMount } from "./owned-mount.mjs";
import { checkImportInventory, createProjectionHost, encodeProjection } from "./owned-projection.mjs";
import { createDirectoryStore } from "./owned-snapshot.mjs";

const textDecoder = new TextDecoder();
const textEncoder = new TextEncoder();

// The WASI configuration both reactors run with; the boundary sizes the
// argv and environment tables from these same values.
const WASI_ARGS = Object.freeze([]);
const WASI_ENV = Object.freeze({});
const wasiStrings = Object.freeze({ args: WASI_ARGS, env: Object.freeze([]) });

function instantiate(module, fs, role, imports = {}) {
  checkImportInventory(module, role, coveredImports);
  const wasi = new WASI({ version: "preview1", args: [...WASI_ARGS], fs, preopens: { "/": "/" }, env: { ...WASI_ENV } });
  let memory;
  const preview1 = boundaryImports(module, wasi.getImportObject().wasi_snapshot_preview1, () => memory, wasiStrings);
  const instance = new WebAssembly.Instance(module, { wasi_snapshot_preview1: preview1, ...imports });
  memory = instance.exports.memory;
  wasi.initialize(instance);
  return instance;
}

const describe = error => (error instanceof Error ? `${error.name}: ${error.message}` : String(error));

/** `length` bytes at an exported u32 `pointer`, checked against `memory`. */
function exported(memory, pointer, length, what) {
  const at = u32(pointer, `${what} pointer`);
  const size = u32(length, `${what} length`);
  if (at + size > memory.buffer.byteLength) throw new RangeError(`${what} [${at}, ${at + size}) is outside memory`);
  return new Uint8Array(memory.buffer, at, size);
}

/**
 * Serves owned checks on `port`.
 *
 * Messages in: `{ type: "init", checker, producer, mount }` (module bytes and
 * a validated mount) or `{ type: "init", checker, producer, directory }`
 * (`{ input, root, identity }`, captured on the main thread), then
 * `{ type: "check", id, requestJson }`.
 * Messages out: `ready`, `init-failed`, `result` (`status`, the reactor's
 * output text, and this check's transport counts), and `poison`, after which
 * nothing more is answered.
 *
 * With a directory, a check's answer is delivered only if the store raised
 * no fault and every record it holds still validates; otherwise the checker
 * is poisoned with the store's own reason, whatever the guest did.
 *
 * A check must also leave no project projection answer pending: one left
 * behind poisons the checker before it is released.
 */
export function serve(port, adapters = {}) {
  let state = "waiting";
  let checker;
  let transport;
  let directory;
  let projection;
  let checks = 0;
  const poison = (id, reason) => {
    state = "poisoned";
    port.postMessage({ type: "poison", id, reason });
    // Terminal cleanup, after the poison is recorded: nothing held survives.
    projection?.clear();
  };
  const initialize = async ({ checker: checkerBytes, producer: producerBytes, mount, directory: source }) => {
    const [checkerModule, producerModule] = await Promise.all([
      WebAssembly.compile(checkerBytes),
      WebAssembly.compile(producerBytes),
    ]);
    let fs;
    if (source) {
      directory = createDirectoryStore({
        input: source.input, root: source.root, rootIdentity: source.identity,
        loopError: message => new WasiError(message, WasiErrno.ELOOP), links: MAX_SUBSTITUTIONS,
      });
      adapters.store?.(directory);
      fs = directory.fs;
    } else {
      const volume = memfs({}, "/");
      materializeMount(volume.vol, mount);
      fs = volume.fs;
    }
    // The project question (S3c): a mount has no host to ask. A directory
    // answer is encoded against the store's own root, an injected one too.
    projection = createProjectionHost(spelling => {
      if (!directory) return encodeProjection({ tag: "unavailable" });
      const compute = () => directory.projectAnswer(spelling);
      return encodeProjection(adapters.projection ? adapters.projection(spelling, compute) : compute(), directory.root);
    });
    const producer = instantiate(producerModule, fs, "producer");
    transport = createTransport(adapters.producer ? adapters.producer(producer.exports) : producer.exports);
    const imports = adapters.imports ? adapters.imports(transport.imports, producer.exports) : transport.imports;
    const projectionImports = adapters.projectionImports ? adapters.projectionImports(projection.imports) : projection.imports;
    checker = instantiate(checkerModule, fs, "checker", { solid_typefacts: protocolImports(imports), solid_owned_host: projectionImports });
    transport.bind(checker.exports.memory);
    projection.bind(checker.exports.memory);
    if (directory?.fault()) throw directory.fault();
  };
  const run = ({ id, requestJson }) => {
    const index = checks++;
    transport.begin();
    adapters.beforeCheck?.(index);
    let status;
    let output;
    let trap;
    try {
      const input = textEncoder.encode(requestJson);
      exported(checker.exports.memory, checker.exports.allocate_input(input.length), input.length, "checker input").set(input);
      status = checker.exports.run_owned_check();
      output = textDecoder.decode(exported(checker.exports.memory, checker.exports.output_pointer(),
        checker.exports.output_length(), "checker output").slice());
    } catch (error) {
      trap = error;
    }
    // The store's fault, then its validation, decide before the guest's own
    // outcome: an answer computed from faulted or changed input is never sent.
    if (directory) {
      const fault = directory.fault() ?? directory.validate();
      if (fault) {
        poison(id, fault.message);
        return;
      }
    }
    if (trap) {
      poison(id, `the check trapped: ${describe(trap)}`);
      return;
    }
    adapters.afterCheck?.(index);
    const stats = transport.stats();
    if (status !== 0 && status !== 1) {
      poison(id, `the checker answered status ${status}`);
      return;
    }
    if (stats.livePrograms.length !== 0 || stats.pendingResults.length !== 0) {
      poison(id, `the check left programs ${JSON.stringify(stats.livePrograms)} and results ${JSON.stringify(stats.pendingResults)} behind`);
      return;
    }
    if (projection.pending()) {
      poison(id, "the check left a project projection answer behind");
      return;
    }
    const { livePrograms: _, ...counts } = stats;
    port.postMessage({ type: "result", id, status, output, transport: counts });
  };
  port.on("message", message => {
    if (state === "waiting" && message?.type === "init") {
      state = "starting";
      initialize(message).then(
        () => {
          state = "ready";
          port.postMessage({ type: "ready" });
        },
        error => {
          state = "failed";
          port.postMessage({ type: "init-failed", error: describe(error) });
        },
      );
    } else if (state === "ready" && message?.type === "check") {
      run(message);
    } else if (state !== "poisoned" && state !== "failed") {
      poison(message?.id, `unexpected ${JSON.stringify(message?.type)} message while ${state}`);
    }
  });
}
