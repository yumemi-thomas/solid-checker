// run.mjs <casesDir> <out.json> [--jobs N] [--unfocus] [--steal] [filter...]
// Serves <casesDir>/<id>/App.tsx through one Vite server (rc.21 install, the fuzzer's Solid plugin options) and runs each in a fresh
// headless Chromium context. Records Solid diagnostics (OBSERVE), uncaught errors, __log lines and document.hasFocus().
// --jobs N workers (default half the cores), each its own browser process, one page open at a time: pages in one browser could
//   disturb each other's focus, and these programs depend on focus. `--jobs 1` is sequential.
// --unfocus disables Playwright's focus emulation (CDP Emulation.setFocusEmulationEnabled false) before the page loads.
// --steal opens a second page in the same context and brings it to the front first.
// Progress goes to stderr as done/total. FLIP_TMP overrides the temp root.
import { readdirSync, existsSync, writeFileSync, mkdirSync, mkdtempSync, realpathSync, rmSync, symlinkSync, readFileSync } from "node:fs";
import { availableParallelism, tmpdir } from "node:os";
import { join, resolve, dirname } from "node:path";
import { createRequire } from "node:module";
import { pathToFileURL, fileURLToPath } from "node:url";

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const INSTALL = process.env.FLIP_INSTALL ?? join(repo, "scripts/runtime-oracle-install"); // rc.21
const here = dirname(fileURLToPath(import.meta.url));
// Real path: on macOS /tmp is a symlink, and Vite then cannot find the page's inline module, so every case times out.
const root = realpathSync(mkdtempSync(join(process.env.FLIP_TMP ?? tmpdir(), "flip-harness-")));
mkdirSync(join(root, "cases"), { recursive: true });
if (!existsSync(join(root, "node_modules"))) symlinkSync(join(INSTALL, "node_modules"), join(root, "node_modules"));

const args = process.argv.slice(2);
const jobsAt = args.indexOf("--jobs");
const jobs = jobsAt < 0 ? Math.max(1, Math.floor(availableParallelism() / 2)) : Number(args.splice(jobsAt, 2)[1]);
if (!Number.isInteger(jobs) || jobs < 1) throw new Error("--jobs takes a positive integer");
const unfocus = args.includes("--unfocus");
const steal = args.includes("--steal");
const [casesDir, out, ...only] = args.filter((a) => a !== "--unfocus" && a !== "--steal");
const ids = readdirSync(casesDir).filter((n) => existsSync(join(casesDir, n, "App.tsx")) && (!only.length || only.some((o) => n.includes(o)))).sort();
for (const id of ids) {
  const dir = join(root, "cases", id);
  mkdirSync(dir, { recursive: true });
  // Every source of the case: a program may span files.
  for (const name of readdirSync(join(casesDir, id))) if (/\.tsx?$/.test(name)) writeFileSync(join(dir, name), readFileSync(join(casesDir, id, name)));
  writeFileSync(join(dir, "entry.tsx"), `import { OBSERVE, flush } from "solid-js";
import { render } from "@solidjs/web";
const rec: any = { diagnostics: [], errors: [], logs: [], hasFocus: document.hasFocus() };
(window as any).__log = (...a: any[]) => rec.logs.push(((window as any).__phase ?? "?") + ": " + a.map(String).join(" "));
window.addEventListener("error", (e) => rec.errors.push(((window as any).__phase ?? "?") + ": " + String((e as any).error?.message ?? e.message)));
window.addEventListener("unhandledrejection", (e: any) => rec.errors.push(((window as any).__phase ?? "?") + ": rejection " + String(e.reason?.message ?? e.reason)));
OBSERVE!.diagnostics.subscribe((ev: any) => rec.diagnostics.push({ phase: (window as any).__phase ?? "?", code: ev.code, message: String(ev.message ?? "").slice(0, 160) }));
// Imported after the subscription, so a diagnostic raised while App's module evaluates is recorded too.
(window as any).__phase = "module";
let mod: any = {};
try { mod = await import("./App"); } catch (e: any) { rec.errors.push("module: " + e.message); }
(window as any).__phase = "render";
try { render(() => <mod.App />, document.getElementById("root")!); } catch (e: any) { rec.errors.push("render: " + e.message); }
try { flush(); } catch (e: any) { rec.errors.push("flush: " + e.message); }
await new Promise((r) => setTimeout(r, 50));
(window as any).__phase = "after";
if ((mod as any).after) { try { await (mod as any).after(); flush(); } catch (e: any) { rec.errors.push("after: " + e.message); } }
await new Promise((r) => setTimeout(r, 100));
rec.hasFocusEnd = document.hasFocus();
(window as any).__rec = rec;
`);
}
writeFileSync(join(root, "index.html"), `<!doctype html><html><body><div id="root"></div><script type="module">
const id = new URLSearchParams(location.search).get("id");
import("/cases/" + id + "/entry.tsx").catch((e) => { window.__rec = { errors: ["entry: " + e.message], diagnostics: [], logs: [] }; });
</script></body></html>`);

const req = createRequire(join(INSTALL, "package.json"));
const vite = await import(pathToFileURL(req.resolve("vite")).href);
const pluginModule = await import(pathToFileURL(req.resolve("@solidjs/vite-plugin")).href);
const solid = pluginModule.default?.default ?? pluginModule.default ?? pluginModule.solid;
const server = await vite.createServer({
  root, configFile: false, logLevel: "error", server: { port: 0, strictPort: false, host: "127.0.0.1" },
  resolve: { preserveSymlinks: false },
  plugins: [solid({ hot: false, performanceTracks: false, diagnostics: false })],
  cacheDir: join(root, ".vite"),
});
await server.listen();
const base = `http://127.0.0.1:${server.config.server.port ?? server.httpServer.address().port}/`;
const playwright = await import(pathToFileURL(req.resolve("playwright-core")).href);
const { pinnedBrowser } = await import(join(repo, "scripts/playwright-browser.mjs"));
const executablePath = pinnedBrowser({ home: process.env.HOME, install: INSTALL }).executable;
const chromium = playwright.chromium ?? playwright.default?.chromium;
const results = {};
async function runCase(browser, id) {
  const context = await browser.newContext();
  const page = await context.newPage();
  const consoleLines = [];
  page.on("console", (m) => { if (m.type() === "error" || m.type() === "warning") consoleLines.push(m.type() + ": " + m.text().slice(0, 200)); });
  if (unfocus) { const cdp = await context.newCDPSession(page); await cdp.send("Emulation.setFocusEmulationEnabled", { enabled: false }); }
  if (steal) { const other = await context.newPage(); await other.goto("about:blank"); await other.bringToFront(); }
  try {
    await page.goto(`${base}index.html?id=${id}`, { waitUntil: "load" });
    await page.waitForFunction(() => window.__rec !== undefined, null, { timeout: 15000 });
    results[id] = await page.evaluate(() => window.__rec);
    results[id].console = consoleLines;
  } catch (e) { results[id] = { failure: String(e.message).slice(0, 300), console: consoleLines }; }
  await context.close();
}
const launch = () => chromium.launch({ executablePath, headless: true, args: ["--in-process-gpu"] });
let next = 0, done = 0;
const every = Math.max(1, Math.floor(ids.length / 20));
const finished = () => { done += 1; if (done === ids.length || done % every === 0) process.stderr.write(`chrome ${done}/${ids.length}\n`); };
// The first case alone: Vite prebundles the Solid packages on the first request and reloads the page when it finishes.
if (ids.length) { const browser = await launch(); await runCase(browser, ids[next++]); await browser.close(); finished(); }
await Promise.all(Array.from({ length: Math.min(jobs, Math.max(0, ids.length - 1)) }, async () => {
  const browser = await launch();
  while (next < ids.length) { const id = ids[next++]; await runCase(browser, id); finished(); }
  await browser.close();
}));
await server.close();
rmSync(root, { recursive: true, force: true });
writeFileSync(out, JSON.stringify(results, null, 1));
for (const id of ids) {
  const r = results[id];
  console.log(`## ${id}  hasFocus=${r.hasFocus}/${r.hasFocusEnd}${r.failure ? "  FAIL " + r.failure : ""}`);
  for (const d of r.diagnostics ?? []) console.log(`   diag ${d.phase} ${d.code}: ${d.message}`);
  for (const e of r.errors ?? []) console.log(`   err  ${e}`);
  for (const l of r.logs ?? []) console.log(`   log  ${l}`);
  for (const c of r.console ?? []) console.log(`   con  ${c}`);
}
process.exit(0);
