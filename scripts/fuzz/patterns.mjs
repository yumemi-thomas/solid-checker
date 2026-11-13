// The pattern lane: the audits' programs, verbatim, in real Chrome and the checker. Each program under patterns/<report>/<id>/ is what a
// review ran and tsc accepted. The generators write shapes, and these are the ones they cannot write yet: each is a held input for the fixes
// (queue.md), judged on every run against open-defects.json (open-defects.mjs). Chrome judges a program by its reported and thrown codes.
//
//   node scripts/fuzz/patterns.mjs [--checker <bin>] [--typefacts <bin>] [--out <dir>] [--only substring[,substring]] [--update]
//
// A false clean: Chrome reports a claimed code and no finding covers it, `certified` when the program has no uncertifiable finding. A false
// positive: a violation of a rule with a runtime counterpart none of whose codes Chrome reported; the lane has no hit counters, so whether
// the site ran is the entry's to say. `--update` rewrites the `patterns` half of the list from this run: for a person, reading the diff.
import { spawn } from "node:child_process";
import { cpSync, existsSync, mkdirSync, readFileSync, readdirSync, realpathSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { AssumedReportError, refuseAssumed } from "../lib/assumed.mjs";
import { NO_RUNTIME_WARNING } from "./checker.mjs";
import { besideCode, codeOfLogged, codeRules, codesForRule, coversCodeAt, ignoredRules, isPlacelessCode, ruleHasRuntimeCounterpart, withheldAtStack, withStarts } from "./codes.mjs";
import { mapPositions } from "./chrome-host.mjs";
import { callStarts, frameStarts } from "./call-starts.mjs";
import { patternClaim, stackedProblems } from "./stacked.mjs";
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

// Each diagnostic and thrown error keeps the case-module frames of its stack as served (`frames`: url, line, column), the oracle's depth
// (41 frames, chrome-host/runner.tsx); runChrome maps them to source lines (`stacks`), so a withholding covers a code only at its site.
const ENTRY = (clickAll) => `import { OBSERVE, flush } from "solid-js";
import { render } from "@solidjs/web";
const rec: any = { diagnostics: [], errors: [], sites: [] };
(globalThis as any).__log = () => {};
const frames = (stack: unknown) => String(stack ?? "").split("\\n").flatMap((text) => {
  const match = /(\\/cases\\/[^/]+\\/[^/?:\\s)]+)(?:\\?[^:\\s)]*)?:(\\d+):(\\d+)/.exec(text);
  return match && !match[1].endsWith("/entry.tsx") ? [{ url: match[1], line: Number(match[2]), column: Number(match[3]) }] : [];
});
const stackHere = () => { const limit = Error.stackTraceLimit; Error.stackTraceLimit = 41; try { return new Error("diagnostic").stack; } finally { Error.stackTraceLimit = limit; } };
const error = (message: unknown, stack: unknown) => { rec.errors.push(String(message)); rec.sites.push({ message: String(message), frames: frames(stack) }); };
window.addEventListener("error", (e) => error((e as any).error?.message ?? e.message, (e as any).error?.stack));
window.addEventListener("unhandledrejection", (e: any) => error(e.reason?.message ?? e.reason, e.reason?.stack));
OBSERVE!.diagnostics.subscribe((ev: any) => { rec.diagnostics.push(ev.code); rec.sites.push({ code: ev.code, frames: frames(stackHere()) }); });
// Imported after the subscription, so a diagnostic raised while App's module evaluates is recorded too. A module that fails
// to load fails the entry, as a static import did: the program did not run.
const mod: any = await import("./App");
try { render(() => <mod.App />, document.getElementById("root")!); } catch (e: any) { error(e.message, e.stack); }
try { flush(); } catch (e: any) { error(e.message, e.stack); }
await new Promise((r) => setTimeout(r, 50));
if ((mod as any).after) { try { await (mod as any).after(); flush(); } catch (e: any) { error(e.message, e.stack); } }
${clickAll ? 'try { for (const b of Array.from(document.querySelectorAll("button"))) (b as any).click(); flush(); } catch (e: any) { error(e.message, e.stack); }' : ""}
await new Promise((r) => setTimeout(r, 100));
(window as any).__rec = rec;
`;

/**
 * The stacks of a record's diagnostics and thrown errors, per code: `{ CODE: [[{ file, line, column, mount? }], …] }`, each innermost first, in
 * source positions through the transform Vite served each case module with. The positions are the grids' (chrome-host.mjs `mapPositions`, the
 * oracle's lookup): a frame at a compiled `insert`/`effect`/`memo` call also records the source start of the expression it was handed, a frame
 * at a compiled `spread` call (no mapping of its own) stands for the spread its `loc` names (ADRs 0921, 0930), an App.tsx frame with no
 * original position is dropped, and another module's frame with none keeps its file and no position (it places nothing and still counts for
 * depth, R20). `starts` are `codes.mjs withStarts`'s over call-starts.mjs `frameStarts`; `mount` marks a frame at a mount of the program or a
 * component (call-starts.mjs). The lane used to drop every unmapped frame and record no compiled start, so a diagnostic Solid raised in a
 * compiled `insert` or `spread` of an element placed nothing at it (the review of #222's NO_OWNER_EFFECT follow-up).
 */
export async function mapStacks(server, lookup, caseDir, rec) {
  const id = basename(caseDir);
  const texts = new Map();
  const textOf = (file) => {
    if (!texts.has(file)) { let text; try { text = readFileSync(join(caseDir, file), "utf8"); } catch {} texts.set(file, text); }
    return texts.get(file);
  };
  const startsOf = (frame, depth) => {
    const text = textOf(frame.file);
    if (text === undefined) return null;
    const starts = frameStarts(text, frame.file)(frame, depth);
    return starts.length > 1 ? starts : null;
  };
  const stacks = {};
  for (const row of rec.sites ?? []) {
    const code = row.code ?? codeOfLogged(row.message) ?? /^\[([A-Z][A-Z_]+)\]/.exec(String(row.message))?.[1];
    if (!code) continue;
    // The served positions as the host records them: an App.tsx frame carries no file, another module's its own.
    const positions = (row.frames ?? []).map((frame) => {
      const file = frame.url.split("/").slice(3).join("/");
      return file === "App.tsx" ? { line: frame.line, column: frame.column } : { file, line: frame.line, column: frame.column };
    });
    const record = { diagnostics: [{ positions }] };
    const mapped = await mapPositions(server, lookup, id, record, textOf("App.tsx"), (position) => position, textOf);
    // A served App.tsx with no map comes back as it went in: its served positions are no source positions, so the stack places nothing.
    const frames = mapped === record ? [] : (mapped.diagnostics?.[0]?.positions ?? []).map((position) => ({ ...position, file: position.file ?? "App.tsx" }));
    const stack = withStarts(frames, startsOf).map((frame) => {
      const text = textOf(frame.file);
      return Number.isInteger(frame.column) && text !== undefined && callStarts(text, frame.file).mount(frame.line, frame.column) ? { ...frame, mount: true } : frame;
    });
    const key = JSON.stringify(stack);
    if (!(stacks[code] ??= []).some((known) => JSON.stringify(known) === key)) stacks[code].push(stack);
  }
  return stacks;
}

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
  const mapping = await import(pathToFileURL(req.resolve("@jridgewell/trace-mapping")).href);
  const lookup = { TraceMap: mapping.TraceMap ?? mapping.default?.TraceMap, originalPositionFor: mapping.originalPositionFor ?? mapping.default?.originalPositionFor, traceSegment: mapping.traceSegment ?? mapping.default?.traceSegment };
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
        const thrown = rec.errors.map((message) => codeOfLogged(message)).filter(Boolean);
        const entry = rec.errors.find((message) => String(message).startsWith("entry: "));
        const stacks = await mapStacks(server, lookup, join(root, "cases", id.replace("/", "--")), rec);
        records.set(id, { codes: [...new Set([...rec.diagnostics, ...thrown])], stacks, ...(entry ? { failure: entry.slice(0, 160) } : {}) });
      } catch (error) { records.set(id, { codes: [], failure: String(error.message).slice(0, 160) }); }
      await context.close();
    }
  }));
  await browser.close();
  await server.close();
  return records;
}

const TSCONFIG = JSON.stringify({ compilerOptions: { jsx: "preserve", jsxImportSource: "@solidjs/web", module: "ESNext", moduleResolution: "Bundler", strict: true, target: "ES2022", lib: ["ES2022", "DOM", "DOM.Iterable"], skipLibCheck: true, noEmit: true }, include: ["*.ts", "*.tsx"] });

/** The checker's findings of each case. Rejects with an `AssumedReportError` when the checker printed a what-if report (ADR 0950): a lane judged on predictions is no lane. */
export async function runChecker(cases, root, { checker, typefacts }) {
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
      try {
        const report = JSON.parse(stdout);
        // The findings are read from `analysis` when the report has one, so that object is held to the same rule.
        refuseAssumed(report, "scripts/fuzz/patterns.mjs");
        refuseAssumed(report?.analysis, "scripts/fuzz/patterns.mjs");
        results.set(id, (report.analysis?.findings ?? report.findings ?? []).filter((finding) => !finding.primaryLocation?.path?.endsWith("main.tsx")));
      } catch (error) {
        if (error instanceof AssumedReportError) throw error;
        results.set(id, null);
      }
    }
  }));
  return results;
}

/** A finding's file and start position, from the checker's JSON (`primaryLocation`) or a judge row. */
const placeOf = (finding) => {
  const at = finding.primaryLocation ?? finding;
  return { path: String(at.path ?? "").replaceAll("\\", "/"), line: at.line, column: at.column };
};
const inFile = (path, file) => file === undefined || path === file || path.endsWith(`/${file}`);

/**
 * Whether a withholding `finding` is at a recorded position of a code (review 4c, R5; codes.mjs `withheldAtStack`): its start is a
 * stack's top frame position (the read, write or throw), or a deeper frame's exact call position on a line that mounts nothing, in
 * that frame's file. Harness frames never reach a stack (only the case's own modules are kept).
 */
export const atRecordedSite = (finding, stacks = [], code = null) => {
  const place = { ...placeOf(finding), rule: finding.rule };
  // A frame of another file is blanked as the grids' `appStack` does (codes.mjs): it keeps its depth and places nothing, its starts included
  // (a compiled or call start in App.tsx at the same line and column is not this finding's place; the review of #224, N1).
  return stacks.some((stack) => withheldAtStack(place, stack.map((frame) => (inFile(place.path, frame.file) ? frame : { file: frame.file, line: -1 })), (frame) => frame.mount === true, code));
};

/**
 * One program's verdict: `{ outcome, codes, certified?, rules? }`. `stacks` (`{ CODE: [[{ file, line, column, mount? }]] }`, innermost
 * first) are the stacks Chrome recorded for each code: a claiming or withholding finding covers a code with a place only at one of them
 * (`atRecordedSite`; review 4c, R5, R8), a placeless code by a claiming finding anywhere and never by a withholding. `positions: false`
 * says the record kept no stacks (the witness rows): a claiming finding then covers anywhere, a withholding only at `stacks`.
 */
export function judgePattern({ codes, failure, stacks = {}, positions = true }, findings) {
  if (failure || findings === null) return { outcome: "no-run", why: failure ?? "no checker report" };
  const relevant = findings.filter((finding) => !ignoredRules.includes(finding.rule));
  const claimed = codes.filter((code) => (codeRules[code] ?? []).length);
  const atSite = (code, finding) => (!positions && (codeRules[code] ?? []).includes(finding.rule)) || atRecordedSite(finding, stacks[code], code);
  const missed = claimed.filter((code) => !relevant.some((finding) => coversCodeAt(code, finding, atSite(code, finding)) || (finding.kind === "violation" && besideCode[code]?.includes(finding.rule))));
  const contradicted = relevant.filter((finding) => finding.kind === "violation" && ruleHasRuntimeCounterpart(finding.rule) && !finding.runtimeSilent && !finding.claimsSilent && !String(finding.hint ?? "").includes(NO_RUNTIME_WARNING) && !codesForRule(finding.rule).some((code) => codes.includes(code)) && !codes.some((code) => besideCode[code]?.includes(finding.rule)));
  if (contradicted.length) return { outcome: "false-positive", codes: claimed, rules: [...new Set(contradicted.map((finding) => finding.rule))] };
  // R8: a missed code whose claiming rule has a violation elsewhere is `misplaced` (the warning in the wrong place, a defect); one Chrome
  // gave no position with a column is `unplaced` (unjudged, never a pass). Any other miss is a false clean, and decides the outcome.
  const placedOf = (code) => (stacks[code] ?? []).some((stack) => stack.some((frame) => frame.handRead || Number.isInteger(frame.column)));
  const misplaced = positions ? missed.filter((code) => !isPlacelessCode(code) && relevant.some((finding) => finding.kind === "violation" && codeRules[code].includes(finding.rule))) : [];
  const unplaced = positions ? missed.filter((code) => !isPlacelessCode(code) && !misplaced.includes(code) && !placedOf(code)) : [];
  const falseClean = missed.filter((code) => !misplaced.includes(code) && !unplaced.includes(code));
  if (falseClean.length) return { outcome: "false-clean", codes: falseClean, certified: !relevant.some((finding) => finding.kind === "uncertifiable") };
  if (misplaced.length) return { outcome: "misplaced", codes: misplaced };
  if (unplaced.length) return { outcome: "unplaced", codes: unplaced };
  return { outcome: "fine", codes: claimed };
}

/** ADR 0696: a program whose rules must stay reported though the run shows nothing: that is its pass, and anything else fails it. */
export function expectedSilent(verdict, rules) {
  if (!rules) return verdict;
  const held = verdict.outcome === "false-positive" && rules.every((rule) => verdict.rules?.includes(rule));
  return held ? { outcome: "fine", codes: [] } : { ...verdict, outcome: "false-clean", why: `expected ${rules.join(", ")} reported` };
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
  const cases = patternCases().filter((entry) => !only || only.split(",").some((part) => entry.id.includes(part)));
  const records = await runChrome(cases, join(out, "chrome"));
  const findings = await runChecker(cases, join(out, "check"), { checker, typefacts });
  const expected = JSON.parse(readFileSync(join(here, "expected-silent.json"), "utf8")).patterns;
  const judged = cases.map(({ id }) => ({ id, ...expectedSilent(judgePattern(records.get(id) ?? { codes: [], failure: "no record" }, findings.get(id) ?? null), expected[id]) }));
  const rows = judged.map((row) => patternClaim(row, judged, records));
  writeFileSync(join(out, "patterns.jsonl"), `${rows.map((row) => JSON.stringify(row)).join("\n")}\n`);
  // Measured, not counted: the false cleans a withholding would cover through a mount line of their stacks (`mountOnly`), or by a frame's
  // line at another column (`lineOnly`).
  const rejudge = (id, frame) => judgePattern({ ...records.get(id), stacks: Object.fromEntries(Object.entries(records.get(id)?.stacks ?? {}).map(([code, list]) => [code, list.map((stack) => stack.map(frame))])) }, findings.get(id) ?? null).outcome === "fine";
  const falseCleans = cases.map(({ id }) => id).filter((id) => rows.find((row) => row.id === id)?.outcome === "false-clean" && records.get(id)?.stacks);
  const mountOnly = falseCleans.filter((id) => rejudge(id, ({ mount, ...frame }) => frame));
  const lineOnly = falseCleans.filter((id) => !mountOnly.includes(id) && rejudge(id, (frame) => ({ ...frame, handRead: true })));
  // The programs the call-start normalization moved: covered with `starts`, not by the frames' own positions alone.
  const byStarts = cases.map(({ id }) => id).filter((id) => rows.find((row) => row.id === id)?.outcome === "fine" && records.get(id)?.stacks && !rejudge(id, ({ starts, ...frame }) => frame));
  writeFileSync(join(out, "deeper.json"), `${JSON.stringify({ mountOnly, lineOnly, byStarts, records: Object.fromEntries(cases.map(({ id }) => [id, records.get(id) ?? null])), findings: Object.fromEntries(cases.map(({ id }) => [id, findings.get(id) ?? null])) }, null, 1)}\n`);
  for (const id of mountOnly) console.log(`MOUNT ${id}: a withholding would cover it only through a mount line`);
  for (const id of lineOnly) console.log(`COLUMN ${id}: a withholding starts on a frame's line at another column`);
  console.log(`call starts: ${byStarts.length} program(s) covered only through a frame's call start (call-starts.mjs)`);
  const list = readOpenDefects();
  if (args.includes("--update")) {
    const before = new Map((list.patterns ?? []).map((entry) => [entry.id, entry]));
    list.patterns = rows.filter((row) => row.outcome !== "fine").map((row) => ({ ...(before.get(row.id) ?? {}), id: row.id, outcome: row.outcome, codes: row.codes ?? [], ...(row.rules ? { rules: row.rules } : {}), ...(row.certified !== undefined ? { certified: row.certified } : {}), ...(row.why ? { why: row.why } : {}) }));
    writeFileSync(OPEN_DEFECTS, `${JSON.stringify(list, null, 1)}\n`);
  }
  const { held, unlisted, fixed, absent } = comparePatterns(rows, readOpenDefects());
  const count = (outcome) => held.filter(({ row }) => row.outcome === outcome).length;
  console.log(`patterns: ${rows.length} programs: ${rows.filter((row) => row.outcome === "fine").length} fine, ${rows.filter((row) => row.outcome === "false-clean").length} false clean, ${rows.filter((row) => row.outcome === "misplaced").length} misplaced, ${rows.filter((row) => row.outcome === "unplaced").length} unplaced`);
  console.log(`known open patterns: ${held.length} still fail (${count("false-positive")} false positive, ${count("false-clean")} false clean, ${count("misplaced")} misplaced, ${count("unplaced")} unplaced, ${count("no-run")} no run; scripts/fuzz/open-defects.json)`);
  for (const row of unlisted) console.log(`NEW ${row.outcome} ${row.id} ${JSON.stringify(row.codes ?? row.why ?? "")}`);
  for (const row of fixed) console.log(`FIXED ${row.id}: move its entry from open-defects.json to rediscovery.json`);
  // The stacked pairs' proofs (stacked-pairs.json): each twin fine, with a code of the pair's rule in Chrome.
  const stackedFailures = stackedProblems(rows, records, undefined, { only });
  for (const problem of stackedFailures) console.log(`STACKED ${problem}`);
  if (!only) for (const id of absent) console.log(`ABSENT ${id}: listed, and no such program`);
  process.exit(unlisted.length || fixed.length || stackedFailures.length || (!only && absent.length) ? 1 : 0);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) await main();
