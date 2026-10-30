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
 * Starts the executor. `chrome` (optional): `{ browser, install, pages }` runs the pages too. Returns `{ prepare(job), run(cases),
 * close() }`; `prepare` and `run` answer as exec/src/main.rs says.
 */
export async function createExecutor({ tsc, bin, env = {}, concurrency, keepWork = false, slotRoot, chrome = null }) {
  build();
  const child = spawn(EXECUTOR, [], { stdio: ["pipe", "pipe", "inherit"] });
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
  child.on("exit", (code, signal) => fail(new Error(`the executor exited (${code ?? signal})`)));
  child.on("error", fail);
  createInterface({ input: child.stdout }).on("line", (line) => {
    const message = JSON.parse(line);
    if (message.op === "ready") { if (message.error) ready.reject(new Error(`the executor did not start: ${message.error}`)); else ready.resolve(message); return; }
    const waiting = pending.get(message.seq);
    if (!waiting) return;
    pending.delete(message.seq);
    if (message.error) waiting.reject(new Error(message.error)); else waiting.resolve(message);
  });
  const send = (message) => child.stdin.write(`${JSON.stringify(message)}\n`);
  send({
    op: "init", tsc, checkerBin: bin, env, concurrency, keepWork, slotRoot,
    chrome: chrome && { node: process.execPath, viteHost: join(here, "exec/vite-host.mjs"), browser: chrome.browser, install: chrome.install, pages: chrome.pages ?? 16, fontFamilies: FONT_FAMILIES },
  });
  await readiness;
  const request = (message) => {
    if (ended) return Promise.reject(ended);
    return new Promise((resolve, reject) => {
      seq += 1;
      pending.set(seq, { resolve, reject });
      send({ ...message, seq });
    });
  };
  return {
    prepare: (job) => request({ op: "prepare", ...job }),
    run: (cases) => request({ op: "run", cases }),
    async close() {
      if (ended) return;
      const exited = new Promise((resolve) => child.once("exit", resolve));
      child.stdin.end();
      await exited;
    },
  };
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
