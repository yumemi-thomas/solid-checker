// One oracle-gate worker thread.
//
// A thread rather than a child process because the cost per case is a
// `ts.createProgram` pair, and a thread gets its own module registry -- hence
// its own `typescript` instance, never shared. A fresh child process per case
// would instead pay TypeScript's module load 161 times.
//
// The worker answers exactly one question, for one case: what did both sides
// say? Every verdict is drawn in the parent, in case order.
import process from "node:process";
import { parentPort, workerData } from "node:worker_threads";

import { runCase } from "./tsc-oracle-case.mjs";

if (!parentPort) throw new Error("tsc-oracle-gate-worker.mjs must be run as a worker thread");

// The id of the case currently being served, so *every* message this thread
// posts -- including the ones from a fault that escapes `runCase` entirely --
// carries the id the parent dispatched. The parent refuses a message whose id
// is not the in-flight task's, and rightly: a result it cannot attribute is a
// result it must not deliver to some other case.
let current = null;

parentPort.on("message", ({ id, payload }) => {
  current = id;
  try {
    const value = runCase(payload.testCase, payload.index, workerData);
    current = null;
    parentPort.postMessage({ id, ok: true, value });
  } catch (error) {
    // Serialized rather than rethrown: an exception crossing the thread
    // boundary as an `error` event would lose which case produced it.
    current = null;
    parentPort.postMessage({
      id,
      ok: false,
      error: {
        message: `${payload.testCase?.rule} [${payload.index}]: ${error?.message ?? error}`,
        stack: error?.stack,
      },
    });
  }
});

// An exception outside the handler above -- an async fault, a native crash path
// -- leaves this thread's state unknown, so it must not serve another case.
// Best effort: attribute the failure to the case in flight, then exit non-zero.
// If the message is lost to the exit, the parent's own `exit` handler still
// rejects that case loudly; either way nothing is replayed and nothing stalls.
process.on("uncaughtException", (error) => {
  try {
    parentPort.postMessage({
      id: current,
      ok: false,
      fatal: true,
      error: { message: `worker fault${current === null ? "" : ` while serving task ${current}`}: ${String(error?.message ?? error)}` },
    });
  } catch {
    // The port is already gone; the exit below is the signal.
  }
  process.exit(1);
});
