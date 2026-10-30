// The Chrome fast host (prototype): the fast path's runner (chrome-host/runner.tsx, a port of fast/runner.tsx) in real Chrome
// instead of Happy DOM. One long-lived Vite dev server (the Solid plugin with the fast path's options) serves every case, one headless
// Chromium runs them, each in a fresh browser context (its own globals, storage and module instances), `concurrency` at a time. No
// checker runs here, and no per-case server or browser launch. The records have the fast path's shape (fast.mjs collectRecords),
// with their App.tsx positions mapped through Vite's source maps (Chrome's Error.stack names positions in the served module).
//
// Same interface as fast-daemon.mjs: `run(cases)` returns `Map<id, record>`, `close()`. `--fast-host chrome` (run.mjs) selects it.
import { existsSync, mkdirSync, readdirSync, rmSync, cpSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { DEFAULT_FAST_INSTALL, installedVersions, pinnedProblems, stageCases } from "./fast.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const DEFAULT_ORACLE_INSTALL = join(here, "../runtime-oracle-install");

/** The oracle's default browser: Playwright's headless shell (scripts/runtime-oracle.mjs `defaultChromium`). */
export function defaultChromium(home = process.env.HOME) {
  const cache = join(home, "Library/Caches/ms-playwright");
  const builds = existsSync(cache) ? readdirSync(cache).filter((name) => /^chromium_headless_shell-\d+$/.test(name)).sort((a, b) => Number(b.split("-")[1]) - Number(a.split("-")[1])) : [];
  if (!builds.length) throw new Error("no playwright chromium headless shell; pass --browser");
  return join(cache, builds[0], "chrome-headless-shell-mac-arm64/chrome-headless-shell");
}

const dbg = (m) => { if (process.env.CHROME_HOST_DEBUG) console.error(`chrome-host: ${m}`); };
const failed = (id, phase, message) => ({ id, failure: { phase, message: String(message).slice(0, 800) }, diagnostics: [], errors: [], witnesses: [] });

/** The clocks a page can run on: Chrome's virtual time over native timers, the runner's JS clock, or the wall clock. */
export const CHROME_CLOCKS = ["virtual-time", "js", "real"];
/** Virtual time granted per budget, and the most one program may use: the runner's waits total about 3 s, a settle at most 2 s more. */
const VIRTUAL_BUDGET_MS = 5000;
const VIRTUAL_CAP_MS = 60000;

/**
 * `clock`: `virtual-time` (default) runs the page's native timers on Chrome's virtual time (`Emulation.setVirtualTimePolicy`,
 * `pauseIfNetworkFetchesPending`): delays, nested-timer clamping, `Date` and task order are Chrome's own, and no wall time is
 * spent waiting; a spent budget is granted again up to VIRTUAL_CAP_MS. `js` is the runner's JS clock (fast/virtual-clock.mjs, a
 * Node-shaped model), `real` the wall clock (as the oracle runs).
 */
export function createChromeHost({ install = DEFAULT_FAST_INSTALL, oracleInstall = DEFAULT_ORACLE_INSTALL, concurrency = 16, clock = "virtual-time", browser: browserPath, pageTimeoutMs = 30000 } = {}) {
  if (!CHROME_CLOCKS.includes(clock)) throw new Error(`--chrome-clock takes ${CHROME_CLOCKS.join(", ")}, not ${clock}`);
  let started = null;
  const start = async () => {
    const root = resolve(install);
    const problems = pinnedProblems(installedVersions(root));
    if (problems.length) throw new Error(`the Chrome host needs the pinned install (${root}): ${problems.join("; ")}`);
    const work = join(root, ".solid-checker-fuzz-chrome", `host-${Date.now()}-${process.pid}`);
    mkdirSync(join(work, "cases"), { recursive: true });
    cpSync(join(here, "chrome-host/runner.tsx"), join(work, "runner.tsx"));
    cpSync(join(here, "chrome-host/index.html"), join(work, "index.html"));
    cpSync(join(here, "fast/virtual-clock.mjs"), join(work, "virtual-clock.mjs"));
    const fromInstall = createRequire(join(root, "package.json"));
    const load = async (name) => import(pathToFileURL(fromInstall.resolve(name)).href);
    const { createServer } = await load("vite");
    // An ES module's default, or a CommonJS module's (one level deeper through a dynamic import of its file).
    const plugin = await load("@solidjs/vite-plugin");
    const solid = [plugin.default, plugin.default?.default, plugin.solid].find((value) => typeof value === "function");
    if (!solid) throw new Error("@solidjs/vite-plugin exports no plugin function");
    const maps = await load("source-map-js");
    const SourceMapConsumer = maps.SourceMapConsumer ?? maps.default?.SourceMapConsumer;
    dbg("loaded tools");
    const server = await createServer({
      configFile: false, root: work, logLevel: "error", mode: "development", cacheDir: join(work, ".vite"),
      plugins: [solid({ hot: false, performanceTracks: false, diagnostics: false, solid: { sourceNames: { locations: true } } })],
      resolve: { conditions: ["browser", "development"], dedupe: ["solid-js", "@solidjs/signals", "@solidjs/web"] },
      server: { host: "127.0.0.1", port: 0, hmr: false, fs: { strict: false }, watch: null },
      optimizeDeps: { include: ["solid-js", "@solidjs/web", "@solidjs/signals", "@solidjs/diagnostics"] },
    });
    dbg("server created");
    await server.listen();
    dbg("listening");
    const base = server.resolvedUrls.local[0];
    const playwright = await import(pathToFileURL(createRequire(join(resolve(oracleInstall), "package.json")).resolve("playwright-core")).href);
    const chromium = playwright.chromium ?? playwright.default?.chromium;
    dbg("playwright loaded");
    const browser = await chromium.launch({ executablePath: browserPath ?? defaultChromium(), headless: true });
    dbg("browser launched");
    // Warm the dependency bundles and the runner once, so no case pays for them or meets Vite's dependency reload.
    const warm = await browser.newContext();
    try { await (await warm.newPage()).goto(`${base}index.html?id=__warm`, { waitUntil: "load" }); } finally { await warm.close(); }
    dbg("warmed");
    return { work, server, base, browser, SourceMapConsumer };
  };

  /** Maps a record's served App.tsx positions to the source's, through the transform Vite served it with. */
  const mapPositions = async (host, id, record) => {
    const url = `/cases/${id}/App.tsx`;
    const module = await host.server.moduleGraph.getModuleByUrl(url);
    const map = module?.transformResult?.map ?? (await host.server.transformRequest(url))?.map;
    if (!map?.mappings) return record;
    const consumer = new host.SourceMapConsumer(map);
    const original = (positions) => (positions ?? []).map((p) => {
      const at = consumer.originalPositionFor({ line: p.line, column: Math.max(0, p.column - 1) });
      return at.line == null ? p : { file: "App.tsx", line: at.line, column: (at.column ?? 0) + 1 };
    });
    return {
      ...record,
      diagnostics: (record.diagnostics ?? []).map((d) => ({ ...d, positions: original(d.positions) })),
      errors: (record.errors ?? []).map((e) => ({ ...e, positions: original(e.positions) })),
    };
  };

  const runOne = async (host, id) => {
    const context = await host.browser.newContext();
    try {
      const page = await context.newPage();
      if (clock === "virtual-time") {
        const cdp = await context.newCDPSession(page);
        let granted = 0;
        const grant = async () => {
          if (granted >= VIRTUAL_CAP_MS) return;
          granted += VIRTUAL_BUDGET_MS;
          await cdp.send("Emulation.setVirtualTimePolicy", { policy: "pauseIfNetworkFetchesPending", budget: VIRTUAL_BUDGET_MS });
        };
        cdp.on("Emulation.virtualTimeBudgetExpired", () => { grant().catch(() => {}); });
        await grant();
      }
      if (process.env.CHROME_HOST_DEBUG) {
        page.on("console", (message) => dbg(`page ${id} console.${message.type()}: ${message.text().slice(0, 300)}`));
        page.on("pageerror", (error) => dbg(`page ${id} error: ${String(error?.stack ?? error).slice(0, 600)}`));
        page.on("requestfailed", (request) => dbg(`page ${id} request failed: ${request.url()} ${request.failure()?.errorText}`));
      }
      await page.goto(`${host.base}index.html?id=${encodeURIComponent(id)}&clock=${clock === "js" ? "virtual" : "real"}`, { waitUntil: "domcontentloaded", timeout: pageTimeoutMs });
      let timer;
      const record = await Promise.race([
        page.evaluate(() => globalThis.__scRecordPromise),
        new Promise((_, reject) => { timer = setTimeout(() => reject(new Error(`no record within ${pageTimeoutMs} ms`)), pageTimeoutMs); }),
      ]).finally(() => clearTimeout(timer));
      return await mapPositions(host, id, record);
    } catch (error) {
      return failed(id, "chrome-host", error?.message ?? error);
    } finally {
      await context.close().catch(() => {});
    }
  };

  return {
    async run(cases) {
      for (const { id } of cases) if (!/^[A-Za-z0-9_-]+$/.test(id)) throw new Error(`unsafe case id ${id}`);
      started ??= start();
      const host = await started;
      stageCases(host.work, cases);
      const records = new Map();
      let next = 0;
      const worker = async () => {
        while (next < cases.length) {
          const { id } = cases[next++];
          records.set(id, await runOne(host, id));
        }
      };
      await Promise.all(Array.from({ length: Math.min(concurrency, cases.length) }, worker));
      for (const { id } of cases) {
        rmSync(join(host.work, "cases", id), { recursive: true, force: true });
        const module = await host.server.moduleGraph.getModuleByUrl(`/cases/${id}/App.tsx`);
        if (module) host.server.moduleGraph.invalidateModule(module);
      }
      rmSync(join(host.work, "tests"), { recursive: true, force: true });
      return records;
    },
    async close() {
      if (!started) return;
      const host = await started.catch(() => null);
      if (!host) return;
      await host.browser.close().catch(() => {});
      await host.server.close().catch(() => {});
      rmSync(host.work, { recursive: true, force: true });
    },
  };
}

/** One case's record written as fast.mjs collects them (the prototype's bench writes these for comparison). */
export const writeRecord = (out, record) => { mkdirSync(out, { recursive: true }); writeFileSync(join(out, `${record.id}.json`), JSON.stringify(record)); };
