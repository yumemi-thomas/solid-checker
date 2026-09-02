import assert from "node:assert/strict";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "vitest";

import { createCliWorkerPool } from "./lib/cli-worker-pool.mjs";

// A stand-in worker speaking the pool protocol. Its behavior is chosen by the
// request's `kind`: echo the arguments, sleep, or die mid-request.
function writeFakeWorker(directory) {
  const path = join(directory, "fake-worker.mjs");
  writeFileSync(path, `
import { createInterface } from "node:readline";
const write = process.stdout.write.bind(process.stdout);
let served = 0;
createInterface({ input: process.stdin }).on("line", async line => {
  const request = JSON.parse(line);
  served += 1;
  if (request.kind === "crash") process.exit(3);
  if (request.kind === "sleep") await new Promise(resolve => setTimeout(resolve, Number(request.args[0])));
  write(JSON.stringify({
    id: request.id,
    status: request.kind === "fail" ? 2 : 0,
    stdout: JSON.stringify({ pid: process.pid, served, args: request.args, env: request.env }),
    stderr: request.kind === "fail" ? "solid-checker: failed\\n" : ""
  }) + "\\n");
}).on("close", () => process.exit(0));
`);
  return path;
}

test("the pool reuses a worker across requests and reports the CLI's status, stdout and stderr", async () => {
  const directory = mkdtempSync(join(tmpdir(), "solid-checker-worker-pool-"));
  const pool = createCliWorkerPool({ maxWorkers: 1, workerScript: writeFakeWorker(directory) });
  try {
    const first = await pool.run({ kind: "echo", args: ["a"], env: { X: "1" } });
    const second = await pool.run({ kind: "fail", args: ["b"], env: { X: null } });
    assert.equal(first.status, 0);
    assert.equal(first.timedOut, false);
    assert.equal(first.memoryExceeded, false);
    const one = JSON.parse(first.stdout);
    const two = JSON.parse(second.stdout);
    assert.equal(one.pid, two.pid, "the same worker served both requests");
    assert.equal(two.served, 2);
    assert.deepEqual(one.args, ["a"]);
    assert.deepEqual(one.env, { X: "1" });
    assert.deepEqual(two.env, { X: null });
    assert.equal(second.status, 2);
    assert.equal(second.stderr, "solid-checker: failed\n");
    assert.equal(pool.size(), 1);
  } finally {
    await pool.close();
    rmSync(directory, { recursive: true, force: true });
  }
});

test("concurrent requests fan out to at most maxWorkers workers and queue beyond", async () => {
  const directory = mkdtempSync(join(tmpdir(), "solid-checker-worker-pool-"));
  const pool = createCliWorkerPool({ maxWorkers: 2, workerScript: writeFakeWorker(directory) });
  try {
    const results = await Promise.all(
      [1, 2, 3, 4].map(index => pool.run({ kind: "sleep", args: [String(60 + index)] }))
    );
    const pids = new Set(results.map(result => JSON.parse(result.stdout).pid));
    assert.equal(pids.size, 2, "two workers, four requests");
    assert.equal(pool.size(), 2);
    assert.ok(results.every(result => result.status === 0));
  } finally {
    await pool.close();
    rmSync(directory, { recursive: true, force: true });
  }
});

test("a timed-out request kills its worker and resolves like a killed CLI child; the next request gets a fresh worker", async () => {
  const directory = mkdtempSync(join(tmpdir(), "solid-checker-worker-pool-"));
  const pool = createCliWorkerPool({ maxWorkers: 1, workerScript: writeFakeWorker(directory) });
  try {
    const before = JSON.parse((await pool.run({ kind: "echo", args: [] })).stdout).pid;
    const slow = await pool.run({ kind: "sleep", args: ["5000"], timeoutMs: 100 });
    assert.equal(slow.timedOut, true);
    assert.equal(slow.status, null);
    const after = JSON.parse((await pool.run({ kind: "echo", args: [] })).stdout).pid;
    assert.notEqual(before, after, "the killed worker was replaced");
  } finally {
    await pool.close();
    rmSync(directory, { recursive: true, force: true });
  }
});

test("a worker that dies mid-request fails only that request", async () => {
  const directory = mkdtempSync(join(tmpdir(), "solid-checker-worker-pool-"));
  const pool = createCliWorkerPool({ maxWorkers: 1, workerScript: writeFakeWorker(directory) });
  try {
    const crashed = await pool.run({ kind: "crash", args: [] });
    assert.equal(crashed.status, 3);
    assert.equal(crashed.timedOut, false);
    assert.match(crashed.stderr, /CLI worker exited \(code 3/);
    const next = await pool.run({ kind: "echo", args: ["ok"] });
    assert.equal(next.status, 0);
  } finally {
    await pool.close();
    rmSync(directory, { recursive: true, force: true });
  }
});

test("memory supervision can kill the worker and marks the request", async () => {
  const directory = mkdtempSync(join(tmpdir(), "solid-checker-worker-pool-"));
  const pool = createCliWorkerPool({
    maxWorkers: 1,
    workerScript: writeFakeWorker(directory),
    supervise: child => {
      const timer = setTimeout(() => child.kill("SIGKILL"), 50);
      return {
        stop: () => clearTimeout(timer),
        exceeded: () => true,
        marker: () => "\n[memory ceiling]"
      };
    }
  });
  try {
    const result = await pool.run({ kind: "sleep", args: ["5000"] });
    assert.equal(result.memoryExceeded, true);
    assert.equal(result.status, null);
    assert.match(result.stderr, /\[memory ceiling\]$/);
  } finally {
    await pool.close();
    rmSync(directory, { recursive: true, force: true });
  }
});

test("workers are recycled after recycleAfter requests", async () => {
  const directory = mkdtempSync(join(tmpdir(), "solid-checker-worker-pool-"));
  const pool = createCliWorkerPool({ maxWorkers: 1, recycleAfter: 2, workerScript: writeFakeWorker(directory) });
  try {
    const a = JSON.parse((await pool.run({ kind: "echo", args: [] })).stdout).pid;
    const b = JSON.parse((await pool.run({ kind: "echo", args: [] })).stdout).pid;
    const c = JSON.parse((await pool.run({ kind: "echo", args: [] })).stdout).pid;
    assert.equal(a, b);
    assert.notEqual(b, c, "a recycled worker is replaced by a fresh one");
  } finally {
    await pool.close();
    rmSync(directory, { recursive: true, force: true });
  }
});

test("the real CLI worker answers an unknown command exactly as the CLI binary would", async () => {
  const pool = createCliWorkerPool({ maxWorkers: 1 });
  try {
    const result = await pool.run({ kind: "frobnicate", args: [] });
    assert.equal(result.status, 2);
    assert.equal(result.stderr, "solid-checker: unknown contract command frobnicate\n");
    const missing = await pool.run({ kind: "generate", args: ["--package-root", "/nonexistent"] });
    assert.equal(missing.status, 2);
    assert.match(missing.stderr, /^solid-checker: --integrity is required/);
  } finally {
    await pool.close();
  }
});
