// The Rust executor (exec/src/main.rs) from the JS side: `--executor rust` (run.mjs). The orchestrator keeps generating programs and
// judging them; the executor writes their projects, type-checks them, runs the checker, and runs their pages in Chrome, on its own
// threads, so none of that waits on (or holds) this process's one thread. Same answers as the JS path: the type checks are
// typecheck.mjs's, the checker's output is parsed here by checker.mjs `checkerResult`, and the records are chrome-host.mjs's (the same
// page, runner, Vite server and browser switches).
import { execFileSync, spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { createInterface } from "node:readline";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { NODE } from "../lib/node-runtime.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const MANIFEST = join(here, "exec/Cargo.toml");
/** The executor binary, built on first use (`cargo build --release`, a no-op when current). */
export const EXECUTOR = join(here, "exec/target/release/fuzz-exec");
/** The executor's sources: part of the Chrome host's record key when it runs the pages (chrome-host.mjs `chromeHostKey`). */
export const EXECUTOR_SOURCES = ["exec.mjs", "exec/Cargo.toml", "exec/Cargo.lock", "exec/src/main.rs", "exec/vite-host.mjs"];

/** Playwright's `Page.setFontFamilies` for chromium on macOS (pw:protocol of chrome-host.mjs's pages): the executor sends the same. */
const FONT_FAMILIES = {
  fontFamilies: { standard: "Times", fixed: "Courier", serif: "Times", sansSerif: "Helvetica", cursive: "Apple Chancery", fantasy: "Papyrus" },
  forScripts: [
    { script: "jpan", fontFamilies: { standard: "Hiragino Kaku Gothic ProN", fixed: "Osaka-Mono", serif: "Hiragino Mincho ProN", sansSerif: "Hiragino Kaku Gothic ProN" } },
    { script: "hang", fontFamilies: { standard: "Apple SD Gothic Neo", serif: "AppleMyungjo", sansSerif: "Apple SD Gothic Neo" } },
    { script: "hans", fontFamilies: { standard: ",PingFang SC,STHeiti", serif: "Songti SC", sansSerif: ",PingFang SC,STHeiti", cursive: "Kaiti SC" } },
    { script: "hant", fontFamilies: { standard: ",PingFang TC,Heiti TC", serif: "Songti TC", sansSerif: ",PingFang TC,Heiti TC", cursive: "Kaiti TC" } },
  ],
};

function build() {
  execFileSync("cargo", ["build", "--release", "--quiet", "--manifest-path", MANIFEST], { stdio: ["ignore", "ignore", "inherit"] });
  if (!existsSync(EXECUTOR)) throw new Error(`the executor did not build: ${EXECUTOR}`);
}

/**
 * How long the executor may take before it is taken for hung: its start (the browser's and the Vite helper's included), one program's
 * step 1 (its type checks and the checker have their own watchdogs), and a Chrome batch (per wave of pages: a page may retry and the
 * browser restart, main.rs `MAX_RESTARTS`). A missed deadline kills the executor and fails every request it holds, so a grid stops
 * with an error instead of waiting forever (a dead browser once held a grid for hours).
 */
export const EXECUTOR_DEADLINES = { readyMs: 300_000, prepareMs: 600_000, runBaseMs: 300_000, runPerWaveMs: 90_000, closeMs: 30_000 };

/**
 * Starts the executor. `chrome` (optional): `{ browser, install, pages }` runs the pages too. `warmChecker`: `off` (a fresh checker
 * per program), `on` (a slot's checks go to a retained checker daemon, one per slot, ADR 0543) or `verify` (`on`, and each warm answer
 * is compared with a one-shot run at the same path: `checker.warm.identical`). Returns `{ prepare(job), run(cases), close() }`;
 * `prepare` and `run` answer as exec/src/main.rs says.
 */
export async function createExecutor({ tsc, bin, env = {}, concurrency, keepWork = false, slotRoot, chrome = null, executable = null, deadlines = EXECUTOR_DEADLINES, warmChecker = "off" }) {
  if (!executable) build();
  const child = spawn(executable ?? EXECUTOR, [], { stdio: ["pipe", "pipe", "inherit"] });
  const pending = new Map();
  let seq = 0;
  let ready;
  const readiness = new Promise((resolve, reject) => { ready = { resolve, reject }; });
  let ended = null;
  const fail = (error) => {
    ended = error;
    ready.reject(error);
    for (const { reject } of pending.values()) reject(error);
    pending.clear();
  };
  const kill = (error) => {
    if (!ended) fail(error);
    if (child.exitCode === null && child.signalCode === null) child.kill("SIGKILL");
  };
  child.on("exit", (code, signal) => { if (!ended) fail(new Error(`the executor exited (${code ?? signal})`)); });
  child.on("error", (error) => kill(error));
  child.stdin.on("error", (error) => kill(new Error(`the executor's input: ${error.message}`)));
  const lines = createInterface({ input: child.stdout });
  lines.on("close", () => { if (!ended) kill(new Error("the executor closed its output")); });
  lines.on("line", (line) => {
    let message;
    try { message = JSON.parse(line); } catch { kill(new Error(`the executor printed ${JSON.stringify(line.slice(0, 200))}`)); return; }
    if (message.op === "ready") { if (message.error) ready.reject(new Error(`the executor did not start: ${message.error}`)); else ready.resolve(message); return; }
    const waiting = pending.get(message.seq);
    if (!waiting) return;
    pending.delete(message.seq);
    if (message.error) waiting.reject(new Error(message.error)); else waiting.resolve(message);
  });
  const send = (message) => child.stdin.write(`${JSON.stringify(message)}\n`);
  send({
    op: "init", tsc, checkerBin: bin, env, concurrency, keepWork, slotRoot, warmChecker,
    chrome: chrome && { node: NODE, viteHost: join(here, "exec/vite-host.mjs"), browser: chrome.browser, install: chrome.install, oracleInstall: chrome.oracleInstall, pages: chrome.pages ?? 16, fontFamilies: FONT_FAMILIES },
  });
  const readyTimer = setTimeout(() => kill(new Error(`the executor was not ready within ${deadlines.readyMs / 1000} s`)), deadlines.readyMs);
  try { await readiness; } finally { clearTimeout(readyTimer); }
  const request = (message, deadlineMs) => {
    if (ended) return Promise.reject(ended);
    return new Promise((resolve, reject) => {
      seq += 1;
      const id = seq;
      const timer = setTimeout(() => kill(new Error(`the executor gave no answer to ${message.op} within ${Math.round(deadlineMs / 1000)} s`)), deadlineMs);
      pending.set(id, { resolve: (value) => { clearTimeout(timer); resolve(value); }, reject: (error) => { clearTimeout(timer); reject(error); } });
      send({ ...message, seq: id });
    });
  };
  const waves = (count) => Math.max(1, Math.ceil(count / (chrome?.pages ?? 16)));
  return {
    prepare: (job) => request({ op: "prepare", ...job }, deadlines.prepareMs),
    run: (cases) => request({ op: "run", cases }, deadlines.runBaseMs + waves(cases.length) * deadlines.runPerWaveMs),
    /** How many checker runs may run at once from now on (at most twice the starting concurrency); resolves to the number set. */
    checks: (count) => request({ op: "checks", count }, deadlines.prepareMs).then((answer) => answer.checks),
    async close() {
      if (child.exitCode !== null || child.signalCode !== null) return;
      const exited = new Promise((resolve) => child.once("exit", resolve));
      child.stdin.end();
      const timer = setTimeout(() => child.kill("SIGKILL"), deadlines.closeMs);
      await exited;
      clearTimeout(timer);
    },
  };
}

/**
 * Sets the Rust executor's checker concurrency to `config.concurrency` (a grid's `run` config: its own plus the idle workers' when
 * every runtime is cached). The executor is sized at its start; without this the grid's larger pool waited on its smaller semaphore
 * (full-8: 10 checkers on 10 vCPUs at ~70% busy where the pool had 20). A config without an executor is left as it is.
 */
export async function setExecutorChecks(config) {
  if (!config.exec) return;
  const executor = await config.exec;
  if (executor.checks) await executor.checks(config.concurrency);
}

/**
 * An executor started on first use: a thenable (`await` it for the executor) that spawns nothing until awaited, as the other fast hosts
 * start on their first batch; a run that never evaluates a program (`--generate-only`) never starts one. `started` says whether it did.
 */
export function lazyExecutor(options) {
  let started = null;
  return {
    get started() { return started !== null; },
    then(resolve, reject) { started ??= createExecutor(options); return started.then(resolve, reject); },
  };
}

/** The Chrome host's interface (`run(cases)`, `timings()`, `close()`) over an executor that runs the pages (`lazyExecutor`). */
export function executorChromeHost(executor) {
  return {
    async run(cases) {
      const answer = await (await executor).run(cases.map(({ id, dir, variant, files }) => ({ id, dir, variant, ...(files ? { files } : {}) })));
      return new Map(Object.entries(answer.records));
    },
    timings: () => ({}),
    async close() { if (executor.started !== false) await (await executor).close(); },
  };
}
