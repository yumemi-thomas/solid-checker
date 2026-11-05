// The pattern lane: the audits' programs, verbatim, in real Chrome and the checker. Each program under patterns/<report>/<id>/ is what a
// review ran and tsc accepted. The generators write shapes, and these are the ones they cannot write yet: each is a held input for the fixes
// (queue.md), judged on every run against open-defects.json (open-defects.mjs). Chrome judges a program by its reported and thrown codes.
//
//   node scripts/fuzz/patterns.mjs [--checker <bin>] [--typefacts <bin>] [--out <dir>] [--only substring] [--update]
//
// A false clean: Chrome reports a claimed code and no finding covers it, `certified` when the program has no uncertifiable finding. A false
// positive: a violation of a rule with a runtime counterpart none of whose codes Chrome reported; the lane has no hit counters, so whether
// the site ran is the entry's to say. `--update` rewrites the `patterns` half of the list from this run: for a person, reading the diff.
import { spawn } from "node:child_process";
import { cpSync, existsSync, mkdirSync, readdirSync, realpathSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { NO_RUNTIME_WARNING } from "./checker.mjs";
import { codeOfThrown, codeRules, codesForRule, coversCode, ignoredRules, ruleHasRuntimeCounterpart } from "./codes.mjs";
import { OPEN_DEFECTS, readOpenDefects } from "./open-defects.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const repo = resolve(here, "../..");
const INSTALL = join(repo, "scripts/runtime-oracle-install");
export const PATTERNS = join(here, "patterns");

/** Every pattern: `{ id, report, dir }`. */
export function patternCases(root = PATTERNS) {
  return readdirSync(root, { withFileTypes: true }).filter((report) => report.isDirectory()).flatMap((report) =>
    readdirSync(join(root, report.name), { withFileTypes: true }).filter((entry) => entry.isDirectory() && existsSync(join(root, report.name, entry.name, "App.tsx")))
      .map((entry) => ({ id: `${report.name}/${entry.name}`, report: report.name, dir: join(root, report.name, entry.name) })));
}

const ENTRY = (clickAll) => `import { OBSERVE, flush } from "solid-js";
import { render } from "@solidjs/web";
const rec: any = { diagnostics: [], errors: [] };
(globalThis as any).__log = () => {};
window.addEventListener("error", (e) => rec.errors.push(String((e as any).error?.message ?? e.message)));
window.addEventListener("unhandledrejection", (e: any) => rec.errors.push(String(e.reason?.message ?? e.reason)));
OBSERVE!.diagnostics.subscribe((ev: any) => rec.diagnostics.push(ev.code));
// Imported after the subscription, so a diagnostic raised while App's module evaluates is recorded too. A module that fails
// to load fails the entry, as a static import did: the program did not run.
const mod: any = await import("./App");
try { render(() => <mod.App />, document.getElementById("root")!); } catch (e: any) { rec.errors.push(e.message); }
try { flush(); } catch (e: any) { rec.errors.push(e.message); }
await new Promise((r) => setTimeout(r, 50));
if ((mod as any).after) { try { await (mod as any).after(); flush(); } catch (e: any) { rec.errors.push(e.message); } }
${clickAll ? 'try { for (const b of Array.from(document.querySelectorAll("button"))) (b as any).click(); flush(); } catch (e: any) { rec.errors.push(e.message); }' : ""}
await new Promise((r) => setTimeout(r, 100));
(window as any).__rec = rec;
`;

/** Chrome's record of each case: `{ codes, failure? }`. */
async function runChrome(cases, root) {
  mkdirSync(join(root, "cases"), { recursive: true });
  if (!existsSync(join(root, "node_modules"))) symlinkSync(join(INSTALL, "node_modules"), join(root, "node_modules"));
  for (const { id, report, dir } of cases) {
    const target = join(root, "cases", id.replace("/", "--"));
    cpSync(dir, target, { recursive: true });
    writeFileSync(join(target, "entry.tsx"), ENTRY(report === "values"));
  }
  writeFileSync(join(root, "index.html"), `<!doctype html><html><body><div id="root"></div><script type="module">
const id = new URLSearchParams(location.search).get("id");
import("/cases/" + id + "/entry.tsx").catch((e) => { window.__rec = { errors: ["entry: " + e.message], diagnostics: [] }; });
</script></body></html>`);
  const req = createRequire(join(INSTALL, "package.json"));
  const vite = await import(pathToFileURL(req.resolve("vite")).href);
  const pluginModule = await import(pathToFileURL(req.resolve("@solidjs/vite-plugin")).href);
  const solid = pluginModule.default?.default ?? pluginModule.default ?? pluginModule.solid;
  const server = await vite.createServer({ root: realpathSync(root), configFile: false, logLevel: "error", server: { port: 0, strictPort: false, host: "127.0.0.1", fs: { strict: false } }, resolve: { preserveSymlinks: false }, optimizeDeps: { noDiscovery: true }, plugins: [solid({ hot: false, performanceTracks: false, diagnostics: false })] });
  await server.listen();
  const base = `http://127.0.0.1:${server.config.server.port ?? server.httpServer.address().port}/`;
  const playwright = await import(pathToFileURL(req.resolve("playwright-core")).href);
  const { pinnedBrowser } = await import(pathToFileURL(join(repo, "scripts/playwright-browser.mjs")).href);
  const chromium = playwright.chromium ?? playwright.default?.chromium;
  const browser = await chromium.launch({ executablePath: pinnedBrowser({ home: process.env.HOME, install: INSTALL }).executable, headless: true, args: ["--in-process-gpu"] });
  const records = new Map();
  let next = 0;
  await Promise.all(Array.from({ length: 6 }, async () => {
    while (next < cases.length) {
      const { id } = cases[next++];
      const context = await browser.newContext();
      const page = await context.newPage();
      if (process.env.SC_PATTERN_DEBUG) { page.on("console", (m) => console.log("page:", m.text().slice(0, 200))); page.on("pageerror", (e) => console.log("pageerror:", String(e).slice(0, 200))); page.on("requestfailed", (r) => console.log("failed:", r.url().slice(0, 140))); page.on("response", (r) => { if (r.status() >= 400) console.log(r.status(), r.url().slice(0, 140)); }); }
      try {
        await page.goto(`${base}index.html?id=${id.replace("/", "--")}`, { waitUntil: "load" });
        await page.waitForFunction(() => window.__rec !== undefined, null, { timeout: 15000 });
        const rec = await page.evaluate(() => window.__rec);
        const thrown = rec.errors.map((message) => codeOfThrown(String(message).replace(/^Error: /, ""))).filter(Boolean);
        const entry = rec.errors.find((message) => String(message).startsWith("entry: "));
        records.set(id, { codes: [...new Set([...rec.diagnostics, ...thrown])], ...(entry ? { failure: entry.slice(0, 160) } : {}) });
      } catch (error) { records.set(id, { codes: [], failure: String(error.message).slice(0, 160) }); }
      await context.close();
    }
  }));
  await browser.close();
  await server.close();
  return records;
}

const TSCONFIG = JSON.stringify({ compilerOptions: { jsx: "preserve", jsxImportSource: "@solidjs/web", module: "ESNext", moduleResolution: "Bundler", strict: true, target: "ES2022", lib: ["ES2022", "DOM", "DOM.Iterable"], skipLibCheck: true, noEmit: true }, include: ["*.ts", "*.tsx"] });

/** The checker's findings of each case. */
async function runChecker(cases, root, { checker, typefacts }) {
  const results = new Map();
  let next = 0;
  await Promise.all(Array.from({ length: 6 }, async () => {
    while (next < cases.length) {
      const { id, dir } = cases[next++];
      const work = join(root, id.replace("/", "--"));
      cpSync(dir, work, { recursive: true });
      mkdirSync(join(work, "node_modules"), { recursive: true });
      for (const name of readdirSync(join(INSTALL, "node_modules"))) if (!existsSync(join(work, "node_modules", name))) symlinkSync(join(INSTALL, "node_modules", name), join(work, "node_modules", name));
      writeFileSync(join(work, "main.tsx"), 'import { render } from "@solidjs/web";\nimport { App } from "./App";\nrender(() => <App />, document.body);\n');
      writeFileSync(join(work, "tsconfig.json"), TSCONFIG);
      const stdout = await new Promise((done) => {
        const child = spawn(checker, ["--project", join(work, "tsconfig.json"), "--format", "json", "--runtime-target", "browser", "--runtime-build", "development", "--rendering", "csr"], { env: { ...process.env, SOLID_TYPEFACTS_BIN: typefacts, SOLID_CHECKER_DAEMON: "0" }, stdio: ["ignore", "pipe", "ignore"] });
        const chunks = [];
        child.stdout.on("data", (chunk) => chunks.push(chunk));
        child.on("close", () => done(Buffer.concat(chunks).toString()));
      });
      try { const report = JSON.parse(stdout); results.set(id, (report.analysis?.findings ?? report.findings ?? []).filter((finding) => !finding.primaryLocation?.path?.endsWith("main.tsx"))); }
      catch { results.set(id, null); }
    }
  }));
  return results;
}

/** One program's verdict: `{ outcome, codes, certified?, rules? }`. */
export function judgePattern({ codes, failure }, findings) {
  if (failure || findings === null) return { outcome: "no-run", why: failure ?? "no checker report" };
  const relevant = findings.filter((finding) => !ignoredRules.includes(finding.rule));
  const claimed = codes.filter((code) => (codeRules[code] ?? []).length);
  const missed = claimed.filter((code) => !relevant.some((finding) => coversCode(code, finding)));
  const contradicted = relevant.filter((finding) => finding.kind === "violation" && ruleHasRuntimeCounterpart(finding.rule) && !finding.runtimeSilent && !finding.claimsSilent && !String(finding.hint ?? "").includes(NO_RUNTIME_WARNING) && !codesForRule(finding.rule).some((code) => codes.includes(code)));
  if (contradicted.length) return { outcome: "false-positive", codes: claimed, rules: [...new Set(contradicted.map((finding) => finding.rule))] };
  if (missed.length) return { outcome: "false-clean", codes: missed, certified: !relevant.some((finding) => finding.kind === "uncertifiable") };
  return { outcome: "fine", codes: claimed };
}

export function comparePatterns(rows, list) {
  const listed = new Map((list.patterns ?? []).map((entry) => [entry.id, entry]));
  const held = [], unlisted = [], fixed = [];
  for (const row of rows) {
    const entry = listed.get(row.id);
    if (row.outcome === "fine") { if (entry) fixed.push(row); continue; }
    if (entry?.outcome === row.outcome) held.push({ row, entry });
    else unlisted.push(row);
  }
  const absent = [...listed.keys()].filter((id) => !rows.some((row) => row.id === id));
  return { held, unlisted, fixed, absent };
}

async function main() {
  const args = process.argv.slice(2);
  const value = (name, fallback) => { const at = args.indexOf(name); return at < 0 ? fallback : args[at + 1]; };
  const checker = value("--checker", join(repo, "rust/target/release/solid-checker-rust"));
  const typefacts = value("--typefacts", join(repo, "bin/solid-typefacts"));
  const only = value("--only");
  const out = value("--out", join(tmpdir(), `solid-checker-patterns-${process.pid}`));
  rmSync(out, { recursive: true, force: true });
  mkdirSync(out, { recursive: true });
  const cases = patternCases().filter((entry) => !only || entry.id.includes(only));
  const records = await runChrome(cases, join(out, "chrome"));
  const findings = await runChecker(cases, join(out, "check"), { checker, typefacts });
  const rows = cases.map(({ id }) => ({ id, ...judgePattern(records.get(id) ?? { codes: [], failure: "no record" }, findings.get(id) ?? null) }));
  writeFileSync(join(out, "patterns.jsonl"), `${rows.map((row) => JSON.stringify(row)).join("\n")}\n`);
  const list = readOpenDefects();
  if (args.includes("--update")) {
    const before = new Map((list.patterns ?? []).map((entry) => [entry.id, entry]));
    list.patterns = rows.filter((row) => row.outcome !== "fine").map((row) => ({ ...(before.get(row.id) ?? {}), id: row.id, outcome: row.outcome, codes: row.codes ?? [], ...(row.rules ? { rules: row.rules } : {}), ...(row.certified !== undefined ? { certified: row.certified } : {}), ...(row.why ? { why: row.why } : {}) }));
    writeFileSync(OPEN_DEFECTS, `${JSON.stringify(list, null, 1)}\n`);
  }
  const { held, unlisted, fixed, absent } = comparePatterns(rows, readOpenDefects());
  const count = (outcome) => held.filter(({ row }) => row.outcome === outcome).length;
  console.log(`patterns: ${rows.length} programs: ${rows.filter((row) => row.outcome === "fine").length} fine`);
  console.log(`known open patterns: ${held.length} still fail (${count("false-positive")} false positive, ${count("false-clean")} false clean, ${count("no-run")} no run; scripts/fuzz/open-defects.json)`);
  for (const row of unlisted) console.log(`NEW ${row.outcome} ${row.id} ${JSON.stringify(row.codes ?? row.why ?? "")}`);
  for (const row of fixed) console.log(`FIXED ${row.id}: move its entry from open-defects.json to rediscovery.json`);
  if (!only) for (const id of absent) console.log(`ABSENT ${id}: listed, and no such program`);
  process.exit(unlisted.length || fixed.length || (!only && absent.length) ? 1 : 0);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) await main();
