// The Chrome fast host (prototype): the fast path's runner (chrome-host/runner.tsx, a port of fast/runner.tsx) in real Chrome
// instead of Happy DOM. One long-lived Vite dev server (the Solid plugin with the fast path's options) serves every case, one headless
// Chromium runs them, each in a fresh browser context (its own globals, storage and module instances), `concurrency` at a time. No
// checker runs here, and no per-case server or browser launch. The records have the fast path's shape (fast.mjs collectRecords),
// with their App.tsx positions mapped through Vite's source maps (Chrome's Error.stack names positions in the served module).
//
// Same interface as fast-daemon.mjs: `run(cases)` returns `Map<id, record>`, `close()`. `--fast-host chrome` (run.mjs) selects it.
import { existsSync, mkdirSync, readdirSync, readFileSync, rmSync, cpSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { DEFAULT_FAST_INSTALL, installedVersions, pinnedProblems, stageCases } from "./fast.mjs";
import { EXECUTOR_SOURCES } from "./exec.mjs";

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
/** How a worker gets a page: a fresh browser context per program, or one page it navigates to a fresh document per program. */
export const CHROME_PAGES = ["fresh-context", "fresh-page", "reuse"];

/**
 * `pages`: `fresh-context` (default) creates a browser context per program; `fresh-page` keeps one context per worker (its HTTP
 * cache warm) and opens a new page per program (a new renderer document and virtual-time state); `reuse` keeps one context and page per worker and
 * navigates it to a fresh document per program (a new realm: globals, module instances, timers and listeners; the HTTP cache and
 * the context's storage persist, which no generated program uses), and recreates them after a failure. `timings()` returns the
 * summed milliseconds of each phase of a program's run.
 */
export function createChromeHost({ install = DEFAULT_FAST_INSTALL, oracleInstall = DEFAULT_ORACLE_INSTALL, concurrency = 16, clock = "virtual-time", pages = "fresh-context", browser: browserPath, pageTimeoutMs = 30000 } = {}) {
  if (!CHROME_CLOCKS.includes(clock)) throw new Error(`--chrome-clock takes ${CHROME_CLOCKS.join(", ")}, not ${clock}`);
  if (!CHROME_PAGES.includes(pages)) throw new Error(`--chrome-pages takes ${CHROME_PAGES.join(", ")}, not ${pages}`);
  const timings = { programs: 0, context: 0, setup: 0, navigate: 0, record: 0, map: 0, teardown: 0 };
  const timed = async (phase, work) => { const at = performance.now(); try { return await work(); } finally { timings[phase] += performance.now() - at; } };
  let started = null;
  const start = async () => {
    const vite = await startViteHost({ install, oracleInstall });
    const playwright = await import(pathToFileURL(createRequire(join(resolve(oracleInstall), "package.json")).resolve("playwright-core")).href);
    const chromium = playwright.chromium ?? playwright.default?.chromium;
    dbg("playwright loaded");
    // The GPU process's compositing in the browser process: a page's frames no longer cross a process boundary (about a fifth of Chrome's
    // system time on macOS; the records of 768 programs were identical with and without it).
    const browser = await chromium.launch({ executablePath: browserPath ?? defaultChromium(), headless: true, args: ["--in-process-gpu"] });
    dbg("browser launched");
    // Warm the dependency bundles and the runner once, so no case pays for them or meets Vite's dependency reload; then freeze the
    // runner and its clock as transformed.
    const warm = await browser.newContext();
    try { await (await warm.newPage()).goto(`${vite.base}index.html?id=__warm`, { waitUntil: "load" }); } finally { await warm.close(); }
    await vite.freeze();
    dbg("warmed");
    return { ...vite, browser };
  };

  /** A context and page ready to run a program: the page's virtual-time session (when used) and its budget reset. */
  const openPage = async (host, existing = null) => {
    const context = existing ?? await timed("context", () => host.browser.newContext());
    const page = await timed("setup", () => context.newPage());
    const slot = { context, page, cdp: null, granted: 0 };
    if (clock === "virtual-time") {
      slot.cdp = await timed("setup", () => context.newCDPSession(page));
      slot.cdp.on("Emulation.virtualTimeBudgetExpired", () => { grant(slot).catch(() => {}); });
    }
    if (process.env.CHROME_HOST_DEBUG) {
      page.on("console", (message) => dbg(`console.${message.type()}: ${message.text().slice(0, 300)}`));
      page.on("pageerror", (error) => dbg(`page error: ${String(error?.stack ?? error).slice(0, 600)}`));
    }
    return slot;
  };
  const grant = async (slot) => {
    if (slot.done || slot.granted >= VIRTUAL_CAP_MS) return;
    slot.granted += VIRTUAL_BUDGET_MS;
    await slot.cdp.send("Emulation.setVirtualTimePolicy", { policy: "pauseIfNetworkFetchesPending", budget: VIRTUAL_BUDGET_MS });
  };
  const closePage = (slot) => timed("teardown", () => slot.context.close().catch(() => {}));

  /** Runs one program on `slot` (a fresh document: navigation replaces the realm); throws when the page fails. */
  const runOn = async (host, slot, id) => {
    // Each program gets a fresh budget before its navigation (a reused page's last program paused virtual time when its record came).
    if (slot.cdp) { slot.granted = 0; slot.done = false; await timed("setup", () => grant(slot)); }
    // A unique query per run: a URL that differed from the last only in its fragment would be a same-document navigation on a reused
    // page (no new document, no new run, the last program's record).
    host.runs = (host.runs ?? 0) + 1;
    await timed("navigate", () => slot.page.goto(`${host.base}sc.html?run=${host.runs}#id=${encodeURIComponent(id)}&clock=${clock === "js" ? "virtual" : "real"}`, { waitUntil: "domcontentloaded", timeout: pageTimeoutMs }));
    let timer;
    const record = await timed("record", () => Promise.race([
      slot.page.evaluate(() => globalThis.__scRecordPromise),
      new Promise((_, reject) => { timer = setTimeout(() => reject(new Error(`no record within ${pageTimeoutMs} ms`)), pageTimeoutMs); }),
    ]).finally(() => clearTimeout(timer)));
    // Done: stop virtual time, so the idle page does not run through budget re-grants (on a reused page that left the next
    // navigation stalled) and no stray timer of this program runs on.
    if (slot.cdp) { slot.done = true; await timed("setup", () => slot.cdp.send("Emulation.setVirtualTimePolicy", { policy: "pause" })); }
    return timed("map", () => host.map(id, record));
  };

  return {
    async run(cases) {
      for (const { id } of cases) if (!/^[A-Za-z0-9_-]+$/.test(id)) throw new Error(`unsafe case id ${id}`);
      started ??= start();
      const host = await started;
      host.stage(cases);
      const records = new Map();
      let next = 0;
      const worker = async () => {
        let slot = null;
        while (next < cases.length) {
          const { id } = cases[next++];
          try {
            slot ??= await openPage(host);
            records.set(id, await runOn(host, slot, id));
          } catch (error) {
            records.set(id, failed(id, "chrome-host", error?.message ?? error));
            if (slot) { await closePage(slot); slot = null; }
          }
          timings.programs += 1;
          if (pages === "fresh-context" && slot) { await closePage(slot); slot = null; }
          if (pages === "fresh-page" && slot) {
            await timed("teardown", () => slot.page.close().catch(() => {}));
            slot = await openPage(host, slot.context).catch(async () => { await closePage(slot); return null; });
          }
        }
        if (slot) await closePage(slot);
      };
      await Promise.all(Array.from({ length: Math.min(concurrency, cases.length) }, worker));
      await host.done(cases.map(({ id }) => id));
      return records;
    },
    timings: () => ({ ...timings }),
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

/**
 * What a Chrome host record depends on beyond the fast path's own sources (cache.mjs `runtimeEnvironmentKey`): the host and its
 * page, runner and clock files, the clock and page mode, and the browser build (every file of its directory, as the oracle's key).
 */
export function chromeHostKey({ clock = "virtual-time", pages = "fresh-context", browser, executor, oracleInstall = DEFAULT_ORACLE_INSTALL } = {}, { fileHash, treeStat }) {
  const executable = browser ?? defaultChromium();
  // The oracle's tools the host compiles and maps with (`startViteHost`).
  const tools = ["vite", "@solidjs/vite-plugin", "@jridgewell/trace-mapping"].map((name) => fileHash(join(resolve(oracleInstall), "node_modules", name, "package.json")));
  return JSON.stringify({
    kind: "chrome", clock, pages, tools, ...(executor ? { executor } : {}),
    sources: ["chrome-host.mjs", "chrome-host/runner.tsx", "chrome-host/index.html", "fast/virtual-clock.mjs", ...(executor === "rust" ? EXECUTOR_SOURCES : [])].map((name) => fileHash(join(here, name))),
    browser: { path: executable, build: treeStat(dirname(executable)) },
  });
}

/** One case's record written as fast.mjs collects them (the prototype's bench writes these for comparison). */
export const writeRecord = (out, record) => { mkdirSync(out, { recursive: true }); writeFileSync(join(out, `${record.id}.json`), JSON.stringify(record)); };

/**
 * The Chrome host's Vite dev server (the Solid plugin with the fast path's options) over a fresh work directory: `{ work, server, base,
 * freeze(), done(ids), map(id, record) }`. Used by `createChromeHost` and by the Rust executor's helper (exec/vite-host.mjs).
 */
export async function startViteHost({ install = DEFAULT_FAST_INSTALL, oracleInstall = DEFAULT_ORACLE_INSTALL } = {}) {
  const root = resolve(install);
  const problems = pinnedProblems(installedVersions(root));
  if (problems.length) throw new Error(`the Chrome host needs the pinned install (${root}): ${problems.join("; ")}`);
  const work = join(root, ".solid-checker-fuzz-chrome", `host-${Date.now()}-${process.pid}`);
  mkdirSync(join(work, "cases"), { recursive: true });
  cpSync(join(here, "chrome-host/runner.tsx"), join(work, "runner.tsx"));
  cpSync(join(here, "chrome-host/index.html"), join(work, "index.html"));
  cpSync(join(here, "fast/virtual-clock.mjs"), join(work, "virtual-clock.mjs"));
  // The oracle's own tools (`feedback run`, packages/cli/scripts/feedback-browser.mjs): its Vite, its Solid plugin with its options,
  // and its source-map lookup, so a page compiles and an event maps to a line exactly as the oracle's do. The served packages are the
  // fast install's (the same pinned Solid release).
  const fromInstall = createRequire(join(resolve(oracleInstall), "package.json"));
  const load = async (name) => import(pathToFileURL(fromInstall.resolve(name)).href);
  const { createServer } = await load("vite");
  // An ES module's default, or a CommonJS module's (one level deeper through a dynamic import of its file).
  const plugin = await load("@solidjs/vite-plugin");
  const solid = [plugin.default, plugin.default?.default, plugin.solid].find((value) => typeof value === "function");
  if (!solid) throw new Error("@solidjs/vite-plugin exports no plugin function");
  const mapping = await load("@jridgewell/trace-mapping");
  const lookup = { TraceMap: mapping.TraceMap ?? mapping.default?.TraceMap, originalPositionFor: mapping.originalPositionFor ?? mapping.default?.originalPositionFor };
  dbg("loaded tools");
  // The page, the runner and its clock, served from memory with long-lived caching once transformed (`freeze`): a program's
  // navigation then fetches only its own modules, and the browser reuses the compiled scripts.
  const cached = new Map();
  const serveCached = {
    name: "solid-checker-fuzz-cached",
    configureServer(server) {
      server.middlewares.use((request, response, next) => {
        const body = cached.get((request.url ?? "").split(/[?#]/)[0]);
        if (body === undefined) return next();
        response.setHeader("Content-Type", body.type);
        response.setHeader("Cache-Control", "max-age=31536000, immutable");
        response.end(body.text);
      });
    },
  };
  // The cases, served from memory (`stage`): their modules through Vite's transform as files at work/cases/<id>/ would be, their plan
  // and oracle files as the static server would serve them. Nothing is written per case, so the grid's file churn (and every watcher
  // of the tree it would wake) stays out of the run. A case that ships a package (its own node_modules) is staged on disk instead:
  // Vite resolves the package from there.
  const casesRoot = join(work, "cases");
  const staged = new Map();
  const stagedFile = (path) => {
    if (!path.startsWith(`${casesRoot}/`)) return undefined;
    const rest = path.slice(casesRoot.length + 1);
    const slash = rest.indexOf("/");
    return slash < 0 ? undefined : staged.get(rest.slice(0, slash))?.get(rest.slice(slash + 1));
  };
  // Vite's default `resolve.extensions`, tried in its order after the exact path.
  const EXTENSIONS = ["", ".mjs", ".js", ".mts", ".ts", ".jsx", ".tsx", ".json"];
  const serveStaged = {
    name: "solid-checker-fuzz-staged",
    enforce: "pre",
    resolveId(source, importer) {
      const [path, query] = source.split("?");
      const base = path.startsWith("/cases/") ? join(work, path)
        : path.startsWith(`${casesRoot}/`) ? path
          : importer && /^\.\.?\//.test(path) && importer.startsWith(`${casesRoot}/`) ? resolve(dirname(importer.split("?")[0]), path) : null;
      if (!base) return null;
      const found = EXTENSIONS.map((extension) => base + extension).find((candidate) => stagedFile(candidate) !== undefined);
      return found === undefined ? null : found + (query ? `?${query}` : "");
    },
    load(id) { return stagedFile(id.split("?")[0]) ?? null; },
    configureServer(server) {
      server.middlewares.use((request, response, next) => {
        const path = (request.url ?? "").split(/[?#]/)[0];
        if (!path.startsWith("/cases/") || /\.(m?[jt]sx?)$/.test(path)) return next();
        const text = stagedFile(join(work, decodeURIComponent(path)));
        if (text === undefined) return next();
        response.setHeader("Content-Type", path.endsWith(".json") ? "application/json" : "text/plain");
        response.end(text);
      });
    },
  };
  const server = await createServer({
    configFile: false, root: work, logLevel: "error", mode: "development", cacheDir: join(work, ".vite"),
    plugins: [serveCached, serveStaged, solid({ hot: false, performanceTracks: false, diagnostics: false })],
    resolve: { conditions: ["browser", "development"], dedupe: ["solid-js", "@solidjs/signals", "@solidjs/web"] },
    // forwardConsole: off whoever launches the run (Vite 8 turns it on under a coding agent; feedback-browser.mjs says why).
    server: { host: "127.0.0.1", port: 0, hmr: false, forwardConsole: false, fs: { strict: false }, watch: null },
    // Only the Solid packages are pre-bundled, at start. Discovery stays off: a case's generated package (every one named `fz-lib`) would
    // be bundled once and served to every later case with another `fz-lib`, and a re-optimization reloads the pages mid-run. A case's
    // package is served from its own node_modules, its `solid-js` import resolved to the same pre-bundled instance.
    optimizeDeps: { noDiscovery: true, include: ["solid-js", "@solidjs/web", "@solidjs/signals", "@solidjs/diagnostics"] },
  });
  dbg("server created");
  await server.listen();
  dbg("listening");
  const base = server.resolvedUrls.local[0];
  // Freezes the page, the runner and its clock as transformed, once a page has loaded them (the dependency bundles are then warm).
  const freeze = async () => {
    const runner = await server.transformRequest("/runner.tsx");
    const virtualClock = await server.transformRequest("/virtual-clock.mjs");
    if (!runner?.code || !virtualClock?.code) throw new Error("the runner did not transform");
    if (!runner.code.includes('from "/virtual-clock.mjs"')) throw new Error("the transformed runner imports its clock from an unexpected URL");
    cached.set("/sc-runner.js", { type: "text/javascript", text: runner.code });
    cached.set("/virtual-clock.mjs", { type: "text/javascript", text: virtualClock.code });
    cached.set("/sc.html", { type: "text/html", text: readFileSync(join(work, "index.html"), "utf8").replace('src="/runner.tsx"', 'src="/sc-runner.js"') });
    // Vite's client (`/@vite/client`), which the @solidjs/web dependency bundle imports for `injectQuery` (Vite rewrites a dynamic
    // import with a computed path through it), is served as that function alone, Vite's own source: the page then opens no HMR
    // WebSocket and loads no overlay (a quarter of Chrome's CPU per program; the records of 768 programs were identical).
    const client = readFileSync(join(fromInstall.resolve("vite/package.json"), "../dist/client/client.mjs"), "utf8");
    const injectQuery = /^function injectQuery\(url, queryToInject\) \{\n[\s\S]*?\n\}\n/m.exec(client)?.[0];
    if (!injectQuery) throw new Error("vite/dist/client/client.mjs no longer defines injectQuery as expected");
    cached.set("/@vite/client", { type: "text/javascript", text: `export ${injectQuery}` });
  };
  /**
   * Stages cases (`{ id, dir, variant, files? }`) where the pages load them: in memory, or on disk for a case that ships a package.
   * The files are fast.mjs `stageCases`'s: App.tsx (the twin's text unless `variant` is clean), its plan and oracle files, its other
   * modules; from `files` (the case's files as built, by relative path) when given, else from `dir`.
   */
  const stage = (cases) => {
    for (const { id, dir, variant, files: given } of cases) {
      const files = given ?? Object.fromEntries(readdirSync(dir).filter((name) => name !== "node_modules").map((name) => [name, readFileSync(join(dir, name), "utf8")]));
      if (Object.keys(files).some((name) => name.startsWith("node_modules/")) || (!given && existsSync(join(dir, "node_modules")))) {
        if (!existsSync(dir)) throw new Error(`a case with a package is staged from its directory: ${dir}`);
        stageCases(work, [{ id, dir, variant }]);
        continue;
      }
      const served = new Map();
      for (const file of ["App.tsx", "sites.json", "oracle.json"]) if (files[file] !== undefined) served.set(file, files[file]);
      for (const file of Object.keys(files).filter((name) => /^[^/]+\.tsx?$/.test(name) && !name.endsWith(".d.ts") && name !== "App.tsx")) served.set(file, files[file]);
      if (variant !== "clean" && files["App.run.txt"] !== undefined) served.set("App.tsx", files["App.run.txt"]);
      staged.set(id, served);
    }
  };
  /** A case staged on disk (one with a package): its App.tsx as served. */
  const readStaged = (id) => { try { return readFileSync(join(casesRoot, id, "App.tsx"), "utf8"); } catch { return undefined; } };
  /** Forgets the cases: their staged files and modules. */
  const done = async (ids) => {
    for (const id of ids) {
      staged.delete(id);
      rmSync(join(casesRoot, id), { recursive: true, force: true });
      const module = await server.moduleGraph.getModuleByUrl(`/cases/${id}/App.tsx`);
      if (module) server.moduleGraph.invalidateModule(module);
    }
  };
  return { work, server, base, freeze, stage, done, map: (id, record) => mapPositions(server, lookup, id, record, staged.get(id)?.get("App.tsx") ?? readStaged(id)) };
}

/**
 * Maps a record's served App.tsx positions to the source's, through the transform Vite served it with, as the oracle does
 * (packages/cli/scripts/feedback-attribution.mjs: `originalPositionFor` at the frame's column, the mapping at or before it on its
 * line); a frame with no original position, or one past the end of its line in `source` (the oracle's `position-out-of-range`), is
 * dropped, never passed through as an App.tsx line.
 */
export async function mapPositions(server, { TraceMap, originalPositionFor }, id, record, source) {
  const url = `/cases/${id}/App.tsx`;
  const module = await server.moduleGraph.getModuleByUrl(url);
  const raw = module?.transformResult?.map ?? (await server.transformRequest(url))?.map;
  if (!raw?.mappings) return record;
  const map = new TraceMap(raw);
  const lines = source === undefined ? null : source.split("\n");
  const original = (positions) => (positions ?? []).flatMap((p) => {
    const at = originalPositionFor(map, { line: p.line, column: Math.max(0, p.column - 1) });
    if (!at.line) return [];
    if (lines && (at.line > lines.length || at.column >= lines[at.line - 1].length)) return [];
    return [{ file: "App.tsx", line: at.line, column: at.column + 1 }];
  });
  return {
    ...record,
    diagnostics: (record.diagnostics ?? []).map((d) => ({ ...d, positions: original(d.positions) })),
    errors: (record.errors ?? []).map((e) => ({ ...e, positions: original(e.positions) })),
  };
}
