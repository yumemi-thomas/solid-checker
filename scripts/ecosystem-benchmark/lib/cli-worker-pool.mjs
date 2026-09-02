// Pool of long-lived CLI workers (see cli-worker.mjs) with the per-request
// guarantees the one-process-per-probe design had: a timeout kills the worker
// serving the request, a memory supervisor may kill it, and either resolves the
// request the way a killed CLI child resolved (status null, `timedOut` /
// `memoryExceeded` set, the supervisor's marker appended to stderr). A worker
// that dies mid-request fails only that request; the next request gets a
// fresh worker. Workers are recycled after `recycleAfter` requests so heap
// drift never accumulates across a whole corpus.

import { spawn as spawnProcess } from "node:child_process";
import { createInterface } from "node:readline";
import { fileURLToPath } from "node:url";

const DEFAULT_WORKER = fileURLToPath(new URL("./cli-worker.mjs", import.meta.url));

export function createCliWorkerPool({
  maxWorkers = 1,
  recycleAfter = 64,
  workerScript = DEFAULT_WORKER,
  executable = process.execPath,
  environment = process.env,
  spawn = spawnProcess,
  supervise = null,
  setTimer = setTimeout,
  clearTimer = clearTimeout
} = {}) {
  const workers = new Set();
  const idle = [];
  const waiting = [];
  let closed = false;
  let nextId = 1;

  const spawnWorker = () => {
    const child = spawn(executable, [workerScript], {
      env: environment,
      stdio: ["pipe", "pipe", "pipe"]
    });
    const worker = { child, busy: null, served: 0, dead: false, stderr: "" };
    const lines = createInterface({ input: child.stdout, crlfDelay: Infinity });
    lines.on("line", line => {
      if (!worker.busy) return;
      let response;
      try {
        response = JSON.parse(line);
      } catch {
        return;
      }
      if (response.id !== worker.busy.id) return;
      worker.busy.settle({
        status: response.status,
        stdout: response.stdout ?? "",
        stderr: response.stderr ?? ""
      });
    });
    child.stderr.on("data", chunk => {
      // The worker's own stderr is protocol-level noise (a crash trace, say);
      // keep the tail so a mid-request death can explain itself.
      worker.stderr = (worker.stderr + chunk).slice(-8192);
    });
    child.on("error", error => {
      worker.stderr += `\n${error.message}`;
    });
    child.on("close", (code, signal) => {
      worker.dead = true;
      workers.delete(worker);
      const index = idle.indexOf(worker);
      if (index >= 0) idle.splice(index, 1);
      if (worker.busy) {
        worker.busy.settle({
          status: code,
          signal,
          stdout: "",
          stderr:
            worker.stderr.trim() ||
            `solid-checker-ecosystem-benchmark: CLI worker exited (code ${code}, signal ${signal}) before answering`
        });
      }
      pump();
    });
    child.unref?.();
    workers.add(worker);
    return worker;
  };

  const acquire = () => {
    while (idle.length) {
      const worker = idle.pop();
      if (!worker.dead) return worker;
    }
    if (workers.size < maxWorkers) return spawnWorker();
    return null;
  };

  const release = worker => {
    if (worker.dead) return;
    worker.served += 1;
    if (worker.served >= recycleAfter) {
      worker.child.stdin.end();
      return;
    }
    idle.push(worker);
  };

  const pump = () => {
    while (waiting.length) {
      const worker = acquire();
      if (!worker) return;
      const job = waiting.shift();
      start(worker, job);
    }
  };

  const start = (worker, job) => {
    const id = nextId++;
    let timedOut = false;
    let settled = false;
    const memory = supervise ? supervise(worker.child) : null;
    const timer = job.timeoutMs
      ? setTimer(() => {
          timedOut = true;
          worker.child.kill("SIGKILL");
        }, job.timeoutMs)
      : null;
    const settle = answer => {
      if (settled) return;
      settled = true;
      if (timer) clearTimer(timer);
      memory?.stop();
      worker.busy = null;
      const memoryExceeded = memory?.exceeded() ?? false;
      job.resolve({
        status: timedOut || memoryExceeded ? null : answer.status,
        stdout: answer.stdout,
        stderr: answer.stderr + (memory?.marker() ?? ""),
        timedOut,
        memoryExceeded
      });
      if (!worker.dead) release(worker);
      pump();
    };
    worker.busy = { id, settle };
    const line = JSON.stringify({ id, kind: job.kind, args: job.args, env: job.env ?? {} });
    try {
      worker.child.stdin.write(`${line}\n`);
    } catch (error) {
      worker.dead = true;
      settle({ status: null, stdout: "", stderr: `could not reach CLI worker: ${error.message}` });
    }
  };

  return {
    run({ kind, args, env, timeoutMs }) {
      if (closed) return Promise.reject(new Error("CLI worker pool is closed"));
      return new Promise(resolve => {
        waiting.push({ kind, args, env, timeoutMs, resolve });
        pump();
      });
    },
    size: () => workers.size,
    async close() {
      closed = true;
      for (const worker of workers) {
        try {
          worker.child.stdin.end();
        } catch {
          // already gone
        }
      }
      await new Promise(resolve => {
        const check = () => (workers.size === 0 ? resolve() : setTimer(check, 20));
        check();
        setTimer(() => {
          for (const worker of workers) worker.child.kill("SIGKILL");
          resolve();
        }, 2000);
      });
    }
  };
}
