// The owned check's worker loop (P5 S2): both reactors on one in-memory
// volume, one check per message, and the transport's ledger checked after
// every check.
//
// The shipped worker (`owned-worker.mjs`) calls `serve(port)` with no
// adapters, and the client posts only `init` and `check` messages, so nothing
// a caller passes to `createOwnedChecker` or `check` reaches `adapters`. The
// adapters exist for the repository's parity gate, which wraps the producer's
// exports or these imports to inject faults as negative controls.
// The pinned @tybys/wasm-util WASI runtime, vendored with physical path
// resolution (vendor/wasm-util/NOTICE). Only this worker uses the copy.
import { WASI } from "./vendor/wasm-util/lib/mjs/wasi/index.mjs";
import { memfs } from "@napi-rs/wasm-runtime/fs";
import { createTransport } from "./owned-host.mjs";
import { materializeMount } from "./owned-mount.mjs";

const textDecoder = new TextDecoder();
const textEncoder = new TextEncoder();

function instantiate(module, fs, imports = {}) {
  const wasi = new WASI({ version: "preview1", fs, preopens: { "/": "/" }, env: {} });
  const instance = new WebAssembly.Instance(module, { ...wasi.getImportObject(), ...imports });
  wasi.initialize(instance);
  return instance;
}

const describe = error => (error instanceof Error ? `${error.name}: ${error.message}` : String(error));

/**
 * Serves owned checks on `port`.
 *
 * Messages in: `{ type: "init", checker, producer, mount }` (module bytes and
 * a validated mount), then `{ type: "check", id, requestJson }`.
 * Messages out: `ready`, `init-failed`, `result` (`status`, the reactor's
 * output text, and this check's transport counts), and `poison`, after which
 * nothing more is answered.
 */
export function serve(port, adapters = {}) {
  let state = "waiting";
  let checker;
  let transport;
  let checks = 0;
  const poison = (id, reason) => {
    state = "poisoned";
    port.postMessage({ type: "poison", id, reason });
  };
  const initialize = async ({ checker: checkerBytes, producer: producerBytes, mount }) => {
    const [checkerModule, producerModule] = await Promise.all([
      WebAssembly.compile(checkerBytes),
      WebAssembly.compile(producerBytes),
    ]);
    const { fs, vol } = memfs({}, "/");
    materializeMount(vol, mount);
    const producer = instantiate(producerModule, fs);
    transport = createTransport(adapters.producer ? adapters.producer(producer.exports) : producer.exports);
    const imports = adapters.imports ? adapters.imports(transport.imports, producer.exports) : transport.imports;
    checker = instantiate(checkerModule, fs, { solid_typefacts: imports });
    transport.bind(checker.exports.memory);
  };
  const run = ({ id, requestJson }) => {
    const index = checks++;
    transport.begin();
    adapters.beforeCheck?.(index);
    let status;
    let output;
    try {
      const input = textEncoder.encode(requestJson);
      const pointer = checker.exports.allocate_input(input.length);
      new Uint8Array(checker.exports.memory.buffer, pointer, input.length).set(input);
      status = checker.exports.run_owned_check();
      output = textDecoder.decode(new Uint8Array(checker.exports.memory.buffer,
        checker.exports.output_pointer(), checker.exports.output_length()).slice());
    } catch (error) {
      poison(id, `the check trapped: ${describe(error)}`);
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
