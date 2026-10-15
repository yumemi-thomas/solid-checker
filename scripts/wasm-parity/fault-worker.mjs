// The parity gate's fault worker: the package's own `serve` loop and
// transport, imported from the installed package, with faults injected only
// at the producer's exports or around the production imports. Never packed;
// every run through it is a negative control, and nothing it answers is
// compared as a fact.
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { parentPort, workerData } from "node:worker_threads";
import { serve } from "solid-checker-wasm/owned-worker-core.mjs";
import { FAULTS, cborOpenEnvelope } from "./cbor.mjs";

const { mode, side, decoyProjectId, outside } = workerData;
const report = message => side.postMessage(message);
const copy = bytes => Buffer.from(bytes);

// Every program the producer issued and released, through the check's
// imports or the decoy opened here, in order.
const programs = { opened: [], closed: [] };
let real;
let lifecycleIndex = 0;
let current;

const output = () => copy(new Uint8Array(real.memory.buffer, real.output_pointer(), real.output_length()));
const handleOf = bytes => bytes.readUInt32LE(0);
function direct(operation, input, ...rest) {
  if (input) {
    const pointer = real.allocate_input(input.length);
    new Uint8Array(real.memory.buffer, pointer, input.length).set(input);
  }
  const status = real[operation](...rest);
  return { status, output: output() };
}

// The producer as the transport sees it: the real exports, except where the
// current control replaces one answer or misroutes one exchange.
function producerAdapter(exports) {
  real = exports;
  let forged;
  const forge = bytes => {
    const buffer = new ArrayBuffer(bytes.length);
    new Uint8Array(buffer).set(bytes);
    forged = { buffer, length: bytes.length };
  };
  return {
    get memory() { return forged ? { buffer: forged.buffer } : real.memory; },
    allocate_input(length) { forged = undefined; return real.allocate_input(length); },
    output_pointer() { return forged ? 0 : real.output_pointer(); },
    output_length() { return forged ? forged.length : real.output_length(); },
    typefacts_open() {
      forged = undefined;
      const status = real.typefacts_open();
      if (status === 0) programs.opened.push(handleOf(output()));
      return status;
    },
    typefacts_handshake() {
      forged = undefined;
      const status = real.typefacts_handshake();
      if (mode === "handshake") {
        const bytes = output();
        bytes[bytes.length - 1] ^= 1;
        forge(bytes);
        if (current) current.faulted = true;
      }
      return status;
    },
    typefacts_lifecycle(handle) {
      forged = undefined;
      lifecycleIndex++;
      const fault = current?.fault;
      if (mode === "trap" && current?.index === 0 && lifecycleIndex === 2) throw new WebAssembly.RuntimeError("injected trap");
      if (fault?.route === "decoy" && !current.faulted && fault.when(lifecycleIndex)) {
        current.faulted = true;
        return real.typefacts_lifecycle(current.decoy.handle);
      }
      const status = real.typefacts_lifecycle(handle);
      if (status === 0 && fault?.apply && !current.faulted) {
        const response = output();
        if (fault.when(lifecycleIndex, response)) {
          forge(fault.apply(response));
          current.faulted = true;
        }
      }
      return status;
    },
    typefacts_close(handle) {
      forged = undefined;
      const status = real.typefacts_close(handle);
      if (status === 0) programs.closed.push(handle);
      return status;
    },
  };
}

// Around the production imports: the worker's exit, a skipped release, or a
// destination one byte too small for the second exchange's answer.
function importsAdapter(imports) {
  return {
    ...imports,
    exchange(handle, pointer, length) {
      if (mode === "exit" && current?.index === 0 && lifecycleIndex === 1) process.exit(7);
      return imports.exchange(handle, pointer, length);
    },
    result_length(handle) {
      const length = imports.result_length(handle);
      return mode === "short-capacity" && current?.index === 0 && lifecycleIndex === 2 && length > 0 ? length - 1 : length;
    },
    close(handle) {
      return mode === "ledger" && current?.index === 0 ? 0 : imports.close(handle);
    },
  };
}

const faultNames = Object.keys(FAULTS);
const adapters = {
  producer: producerAdapter,
  imports: importsAdapter,
  beforeCheck(index) {
    lifecycleIndex = 0;
    if (mode === "error") throw new Error("injected error");
    current = { index, faulted: false };
    // The W5 series: each fault, then a clean check on the same instances.
    if (mode === "faults" && index % 2 === 0) {
      const name = faultNames[index / 2];
      current.name = name;
      current.fault = FAULTS[name];
      if (current.fault.route === "decoy") {
        const opened = direct("typefacts_open", cborOpenEnvelope(decoyProjectId));
        if (opened.status !== 0) throw new Error(`decoy open failed: ${opened.output}`);
        current.decoy = { handle: handleOf(opened.output) };
        programs.opened.push(current.decoy.handle);
      }
    }
  },
  afterCheck() {
    if (current?.decoy) {
      const status = real.typefacts_close(current.decoy.handle);
      current.decoy.closeStatus = status;
      if (status === 0) programs.closed.push(current.decoy.handle);
    }
    const active = programs.opened.filter(handle => !programs.closed.includes(handle));
    report({
      type: "check",
      index: current?.index,
      name: current?.name,
      faulted: current?.faulted ?? false,
      decoy: current?.decoy,
      activePrograms: active,
      programs: { opened: [...programs.opened], closed: [...programs.closed] },
    });
  },
};

if (mode === "canary") {
  // Under the Node permission model a worker may neither start a process nor
  // read outside the granted trees.
  const attempt = action => {
    try {
      action();
      return "allowed";
    } catch (error) {
      return error.code ?? error.name;
    }
  };
  report({
    type: "canary",
    spawn: attempt(() => {
      const result = spawnSync(process.execPath, ["-e", "0"]);
      if (result.error) throw result.error;
    }),
    read: attempt(() => readFileSync(outside)),
  });
}

// Around the worker's own reply: the first result's answer replaced, so the
// client receives output no reactor wrote.
function forgingPort(port) {
  let forged = false;
  return {
    on: (event, listener) => port.on(event, listener),
    postMessage(message, transfer) {
      if (!forged && message?.type === "result") {
        forged = true;
        if (mode === "malformed-json") message = { ...message, output: "{" };
        else if (mode === "malformed-shape") message = { ...message, status: 0, output: '{"error":"forged"}' };
        else message = { ...message, status: 7 };
      }
      return port.postMessage(message, transfer);
    },
  };
}

serve(mode.startsWith("malformed-") ? forgingPort(parentPort) : parentPort, mode === "canary" ? {} : adapters);
