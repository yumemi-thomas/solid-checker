// The fast path with a long-lived Vitest host (fast/serve.mjs): the Vite server, the Solid plugin and its
// transform cache start once and serve every batch, instead of a fresh Vitest process per batch (the shrinker
// asks for one program at a time). Same runner, same config, same records as `runFast`; each request stages its
// cases under its own `cases/r<N>` and `tests/r<N>` so no module path is ever reused (no stale transform), and
// removes them afterwards. Isolation between programs is Vitest's `isolate: true` (a fresh environment per test
// file); fast-daemon.test.mjs proves it with a program that leaves state and the one that follows.
import { spawn } from "node:child_process";
import { existsSync, mkdirSync, rmSync } from "node:fs";
import { join, resolve } from "node:path";
import { DEFAULT_FAST_INSTALL, collectRecords, installedVersions, pinnedProblems, prepareWork, stageCases } from "./fast.mjs";

export function createFastDaemon({ install = DEFAULT_FAST_INSTALL, workers = 4, pauseMs, finalMs, requestTimeoutMs = 600000 } = {}) {
  const root = resolve(install);
  let child = null;
  let ready = null;
  let work = null;
  let nextRequest = 0;
  let queue = Promise.resolve();
  const waiting = new Map();
  let log = "";

  function start() {
    if (ready) return ready;
    const problems = pinnedProblems(installedVersions(root));
    if (problems.length) throw new Error(`the fast path needs the pinned install (${root}): ${problems.join("; ")}`);
    if (!existsSync(join(root, "node_modules/vitest/dist/node.js"))) throw new Error(`no vitest in ${root}/node_modules (npm install in scripts/fuzz/install)`);
    work = prepareWork(root, `daemon-${Date.now()}-${process.pid}`);
    child = spawn(process.execPath, [join(work, "serve.mjs")], {
      cwd: work, stdio: ["pipe", "pipe", "pipe"],
      env: {
        ...process.env, NODE_ENV: "development", FUZZ_FAST_WORKERS: String(workers),
        ...(pauseMs === undefined ? {} : { FUZZ_PAUSE_MS: String(pauseMs) }), ...(finalMs === undefined ? {} : { FUZZ_FINAL_MS: String(finalMs) }),
      },
    });
    ready = new Promise((done, fail) => {
      let buffer = "";
      child.stdout.on("data", (chunk) => {
        buffer += chunk;
        for (let at = buffer.indexOf("\n"); at >= 0; at = buffer.indexOf("\n")) {
          const text = buffer.slice(0, at);
          buffer = buffer.slice(at + 1);
          if (!text.startsWith("@@")) { log = (log + text + "\n").slice(-4000); continue; }
          const message = JSON.parse(text.slice(2));
          if (message.ready) done();
          else waiting.get(message.id)?.(message);
        }
      });
      child.stderr.on("data", (chunk) => { log = (log + chunk).slice(-4000); });
      child.on("error", fail);
      child.on("close", (status) => {
        fail(new Error(`the fast host exited (${status}): ${log.slice(-400)}`));
        for (const resolveRequest of waiting.values()) resolveRequest({ ok: false, error: `the fast host exited (${status})` });
      });
    });
    return ready;
  }

  async function runBatch(cases) {
    for (const { id } of cases) if (!/^[A-Za-z0-9_-]+$/.test(id)) throw new Error(`unsafe case id ${id}`);
    await start();
    const number = nextRequest++;
    const sub = `r${number}`;
    const out = join(work, "out", sub);
    mkdirSync(out, { recursive: true });
    const tests = stageCases(work, cases, { sub, out });
    const id = number;
    const reply = await new Promise((done) => {
      const timer = setTimeout(() => done({ ok: false, error: `no reply within ${requestTimeoutMs} ms` }), requestTimeoutMs);
      waiting.set(id, (message) => { clearTimeout(timer); done(message); });
      child.stdin.write(`${JSON.stringify({ id, tests })}\n`);
    });
    waiting.delete(id);
    const records = collectRecords(cases, out, { status: reply.ok ? "ok" : reply.error, log });
    for (const path of [join(work, "cases", sub), join(work, "tests", sub), out]) rmSync(path, { recursive: true, force: true });
    return records;
  }

  return {
    /** Same contract as `runFast`: `Map<id, record>`. Requests are served one at a time, in order. */
    run(cases) {
      const turn = queue.then(() => runBatch(cases));
      queue = turn.catch(() => {});
      return turn;
    },
    async close() {
      if (!child) return;
      const host = child;
      host.stdin.end();
      await new Promise((done) => {
        const timer = setTimeout(() => { host.kill("SIGKILL"); done(); }, 5000);
        host.on("close", () => { clearTimeout(timer); done(); });
      });
      rmSync(work, { recursive: true, force: true });
      child = null; ready = null;
    },
  };
}
