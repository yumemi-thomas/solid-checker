// run.mjs <casesDir> <out.json> [--unfocus] [filter...]
// Serves <casesDir>/<id>/App.tsx through Vite (rc.21 install, the fuzzer's Solid plugin options) and runs each in a fresh
// headless Chromium context. Records Solid diagnostics (OBSERVE), uncaught errors, __log lines and document.hasFocus().
// --unfocus disables Playwright's focus emulation (CDP Emulation.setFocusEmulationEnabled false) before the page loads.
import { readdirSync, existsSync, writeFileSync, mkdirSync, symlinkSync, readFileSync } from "node:fs";
import { join, resolve, dirname } from "node:path";
import { createRequire } from "node:module";
import { pathToFileURL, fileURLToPath } from "node:url";

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const INSTALL = process.env.FLIP_INSTALL ?? join(repo, "scripts/runtime-oracle-install"); // rc.21
const here = dirname(fileURLToPath(import.meta.url));
const root = join(process.env.FLIP_TMP ?? "/tmp", "flip-harness");
mkdirSync(join(root, "cases"), { recursive: true });
if (!existsSync(join(root, "node_modules"))) symlinkSync(join(INSTALL, "node_modules"), join(root, "node_modules"));

const args = process.argv.slice(2);
const unfocus = args.includes("--unfocus");
const steal = args.includes("--steal");
const [casesDir, out, ...only] = args.filter((a) => a !== "--unfocus" && a !== "--steal");
const ids = readdirSync(casesDir).filter((n) => existsSync(join(casesDir, n, "App.tsx")) && (!only.length || only.some((o) => n.includes(o)))).sort();
for (const id of ids) {
  const dir = join(root, "cases", id);
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, "App.tsx"), readFileSync(join(casesDir, id, "App.tsx")));
  writeFileSync(join(dir, "entry.tsx"), `import { OBSERVE, flush } from "solid-js";
import { render } from "@solidjs/web";
import * as mod from "./App";
const rec: any = { diagnostics: [], errors: [], logs: [], hasFocus: document.hasFocus() };
(window as any).__log = (...a: any[]) => rec.logs.push(((window as any).__phase ?? "?") + ": " + a.map(String).join(" "));
window.addEventListener("error", (e) => rec.errors.push(((window as any).__phase ?? "?") + ": " + String((e as any).error?.message ?? e.message)));
window.addEventListener("unhandledrejection", (e: any) => rec.errors.push(((window as any).__phase ?? "?") + ": rejection " + String(e.reason?.message ?? e.reason)));
OBSERVE!.diagnostics.subscribe((ev: any) => rec.diagnostics.push({ phase: (window as any).__phase ?? "?", code: ev.code, message: String(ev.message ?? "").slice(0, 160) }));
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
});
await server.listen();
const base = `http://127.0.0.1:${server.config.server.port ?? server.httpServer.address().port}/`;
const playwright = await import(pathToFileURL(req.resolve("playwright-core")).href);
const { pinnedBrowser } = await import(join(repo, "scripts/playwright-browser.mjs"));
const executablePath = pinnedBrowser({ home: process.env.HOME, install: INSTALL }).executable;
const chromium = playwright.chromium ?? playwright.default?.chromium;
const browser = await chromium.launch({ executablePath, headless: true, args: ["--in-process-gpu"] });
const results = {};
for (const id of ids) {
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
await browser.close();
await server.close();
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
