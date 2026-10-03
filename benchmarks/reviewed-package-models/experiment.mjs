import assert from "node:assert/strict";
import { existsSync, mkdirSync, readFileSync, symlinkSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { performance } from "node:perf_hooks";
import { spawnSync } from "node:child_process";
import { oracleCompilerOptions } from "../../scripts/tsc-oracle.mjs";
import { hash, read } from "./catalog.mjs";
import { lower, projectWarning, ts } from "./lower.mjs";
import { demandSpecializer, installedCatalog } from "./demand-models.mjs";
const here = dirname(fileURLToPath(import.meta.url)), repo = resolve(here, "../..");
const [runArgument, outArgument, filter = ""] = process.argv.slice(2);
assert(runArgument && outArgument, "Usage: node experiment.mjs <retained-browser-run.json> <fresh-output-directory> [case-id,...]");
const run = read(resolve(runArgument)), out = resolve(outArgument);
assert(!existsSync(out), "Evidence output already exists"); mkdirSync(out, { recursive: true });
const checker = resolve(process.env.SOLID_CHECKER_NATIVE_BIN ?? join(repo, "rust/target/release/solid-checker-rust"));
const typefacts = resolve(process.env.SOLID_TYPEFACTS_BIN ?? join(repo, "bin/solid-typefacts"));
assert(existsSync(checker) && existsSync(typefacts), "Existing checker and Type Facts producer are required");
const catalogPath = resolve(process.env.REVIEWED_MODEL_CATALOG ?? join(here, "models.json"));
const catalog = read(catalogPath);
assert.equal(catalog.authority, false); assert.equal(catalog.format, "solid-checker-reviewed-model-experiment");
const ledger = read(join(repo, "fixtures/primitives-misuse/cases.json")).cases;
const caseCatalog = read(resolve(process.env.REVIEWED_MODEL_CASE_CATALOG ?? join(here, "models.json")));
const selected = ledger.filter(entry => caseCatalog.models.some(model => model.package === entry.package && model.exports[entry.export]));
const extras = [
  { id: "control-alias-read", package: "@solid-primitives/media", export: "createMediaQuery", rule: "strict-read-untracked",
    misuse: `import { createMediaQuery as query } from "@solid-primitives/media";
export default function App() { const read = query("(min-width: 1px)"); const value = read(); return <p>{String(value)}</p>; }`,
    correct: `import { createMediaQuery as query } from "@solid-primitives/media";
export default function App() { const read = query("(min-width: 1px)"); return <p>{String(read())}</p>; }` },
  { id: "control-namespace-read", package: "@solid-primitives/raf", export: "createRAF", rule: "strict-read-untracked",
    misuse: `import * as raf from "@solid-primitives/raf";
export default function App() { const [read] = raf.createRAF(() => {}); const value = read(); return <p>{String(value)}</p>; }`,
    correct: `import * as raf from "@solid-primitives/raf";
export default function App() { const [read] = raf.createRAF(() => {}); return <p>{String(read())}</p>; }` },
  { id: "control-shadowed-factory", package: "@solid-primitives/raf", export: "createRAF", rule: "strict-read-untracked", expectWarning: false,
    misuse: `import { createRAF } from "@solid-primitives/raf";
export default function App() { function createRAF(_fn: () => void) { return [() => false] as const; } const [read] = createRAF(() => {}); const value = read(); return <p>{String(value)}</p>; }` },
  { id: "control-computed-namespace", package: "@solid-primitives/raf", export: "createRAF", rule: "strict-read-untracked", expectWarning: false, expectUnsupported: true,
    misuse: `import * as raf from "@solid-primitives/raf";
export default function App() { const [read] = raf["createRAF"](() => {}); const value = read(); return <p>{String(value)}</p>; }` },
  { id: "control-access-positive-arity", package: "@solid-primitives/utils", export: "access", rule: "strict-read-untracked", expectWarning: false, expectUnsupported: true,
    misuse: `import { access } from "@solid-primitives/utils";
export default function App() { const fn = (value: number) => value; const value = access(fn); return <p>{String(value)}</p>; }` },
  { id: "control-access-length-getter", package: "@solid-primitives/utils", export: "access", rule: "strict-read-untracked", expectWarning: false, expectUnsupported: true,
    misuse: `import { createSignal } from "solid-js"; import { access } from "@solid-primitives/utils";
export default function App() { const [read] = createSignal(1); Object.defineProperty(read, "length", { get: () => 1 }); const value = access(read); return <p>{String(value)}</p>; }` },
  { id: "control-accessor-rebinding", package: "@solid-primitives/media", export: "createMediaQuery", rule: "strict-read-untracked", expectWarning: false, expectUnsupported: true,
    misuse: `import { createMediaQuery } from "@solid-primitives/media";
export default function App() { let read = createMediaQuery("(min-width: 1px)"); read = () => false; const value = read(); return <p>{String(value)}</p>; }` },
  { id: "control-wrapper-return", package: "@solid-primitives/media", export: "createMediaQuery", rule: "strict-read-untracked",
    misuse: `import { createMediaQuery } from "@solid-primitives/media";
function helper() { return createMediaQuery("(min-width: 1px)"); }
export default function App() { const read = helper(); const value = read(); return <p>{String(value)}</p>; }`,
    correct: `import { createMediaQuery } from "@solid-primitives/media";
function helper() { return createMediaQuery("(min-width: 1px)"); }
export default function App() { const read = helper(); return <p>{String(read())}</p>; }` },
  { id: "control-tracked-callback-write", package: "@solid-primitives/memo", export: "createLazyMemo", rule: "reactive-write-in-owned-scope",
    misuse: `import { createSignal } from "solid-js"; import { createLazyMemo } from "@solid-primitives/memo";
export default function App() { const [count, setCount] = createSignal(1); const doubled = createLazyMemo(() => { setCount(count() + 1); return count() * 2; }); return <p>{doubled()}</p>; }`,
    correct: `import { createSignal } from "solid-js"; import { createLazyMemo } from "@solid-primitives/memo";
export default function App() { const [count] = createSignal(1); const doubled = createLazyMemo(() => count() * 2); return <p>{doubled()}</p>; }` },
  { id: "control-deferred-raf-read", package: "@solid-primitives/raf", export: "createRAF", rule: "strict-read-untracked", expectWarning: false,
    misuse: `import { createSignal } from "solid-js"; import { createRAF } from "@solid-primitives/raf";
export default function App() { const [count] = createSignal(1); const [running] = createRAF(() => { console.log(count()); }); return <p>{String(running())}</p>; }` },
  { id: "control-deferred-timer-read", package: "@solid-primitives/timer", export: "createTimer", rule: "strict-read-untracked", expectWarning: false,
    misuse: `import { createSignal } from "solid-js"; import { createTimer } from "@solid-primitives/timer";
export default function App() { const [count] = createSignal(1); createTimer(() => console.log(count()), 1000, setInterval); return <p>timer</p>; }` },
];
const further = process.env.REVIEWED_MODEL_EXTRA_CASES ? (await import(pathToFileURL(resolve(process.env.REVIEWED_MODEL_EXTRA_CASES)).href)).default : [];
const cases = [...selected, ...extras, ...further].filter(entry => !filter || filter.split(",").includes(entry.id));
assert(cases.length, "No matching cases");
assert.equal(new Set(cases.map(entry => entry.id)).size, cases.length, "Duplicate case IDs");
const report = { format: "solid-checker-reviewed-model-experiment-results", version: 1, authority: false,
  checkerSha256: hash(readFileSync(checker)), typefactsSha256: hash(readFileSync(typefacts)), modelSha256: hash(readFileSync(catalogPath)), catalogPath,
  typescript: ts.version, node: process.version, startedAt: new Date().toISOString(), results: [], summary: null };
const baselinePath = process.env.REVIEWED_MODEL_BASELINE_REPORT ? resolve(process.env.REVIEWED_MODEL_BASELINE_REPORT) : null;
const savedBaseline = baselinePath ? read(baselinePath) : null;
if (savedBaseline) {
  for (const key of ["checkerSha256", "typefactsSha256", "typescript", "node"]) assert.equal(savedBaseline[key], report[key], `Historical baseline input changed: ${key}`);
  report.historicalBaseline = baselinePath;
}
function analyze(project, host) {
  const started = performance.now();
  const response = spawnSync(checker, ["--format", "json", "--runtime-target", host, "--project", project], {
    cwd: repo, env: { ...process.env, SOLID_TYPEFACTS_BIN: typefacts, SOLID_CHECKER_DAEMON: "0" }, encoding: "utf8", maxBuffer: 16 * 1024 * 1024, timeout: 30000,
  });
  assert(!response.error, response.error?.message);
  assert([0, 1].includes(response.status), response.stderr);
  const parsed = JSON.parse(response.stdout);
  return { ...parsed, durationMs: performance.now() - started };
}
for (const entry of cases) {
  const retained = run.results.find(row => row.package === entry.package && row.version === caseCatalog.models.find(model => model.package === entry.package).version);
  assert(retained, `Missing cached exact package: ${entry.package}`);
  const dir = join(out, entry.id); mkdirSync(dir);
  symlinkSync(join(retained.retainedArtifacts.projectDir, "node_modules"), join(dir, "node_modules"), "dir");
  const item = { id: entry.id, package: entry.package, export: entry.export, class: entry.class ?? "control", rule: entry.rule,
    group: catalog.models.findIndex(model => model.package === entry.package) < 4 ? "development" : "extension", observations: [] };
  report.results.push(item);
  for (const host of ["browser", ...(caseCatalog.models.find(model => model.package === entry.package).exports[entry.export]?.node?.inert ? ["node"] : [])]) {
    const localCatalog = process.env.REVIEWED_MODEL_ON_DEMAND === "1" ?
      installedCatalog(dir, [{ package: entry.package, exports: [entry.export] }], host) : catalog;
    const specialize = process.env.REVIEWED_MODEL_ON_DEMAND === "1" ? demandSpecializer(localCatalog, dir) : null;
    if (specialize) {
      assert.equal(localCatalog.packages.filter(item => item.error).length, 0, "On-demand inputs were refused");
      writeFileSync(join(dir, `${host}-catalog.json`), JSON.stringify(localCatalog, null, 2));
    }
    for (const twin of ["misuse", "correct"]) {
      if (!entry[twin]) continue;
      const text = entry[twin], originalPath = join(dir, `${host}-${twin}.tsx`), modeledPath = join(dir, `${host}-${twin}-modeled.tsx`);
      writeFileSync(originalPath, text);
      const options = oracleCompilerOptions("v2", true, { customConditions: [host, "development"] });
      const converted = ts.convertCompilerOptionsFromJson(options, dir).options;
      const started = performance.now(), program = ts.createProgram([originalPath], converted);
      const errors = ts.getPreEmitDiagnostics(program).filter(d => d.category === ts.DiagnosticCategory.Error);
      assert.equal(errors.length, 0, `${entry.id}/${twin}: published typings reject specimen: ${ts.formatDiagnostics(errors, {
        getCanonicalFileName: x => x, getCurrentDirectory: () => dir, getNewLine: () => "\n" })}`);
      const source = program.getSourceFile(originalPath), lowered = lower(program, source, localCatalog, host, undefined, specialize);
      const preparationMs = performance.now() - started;
      writeFileSync(modeledPath, lowered.text);
      writeFileSync(join(dir, `${host}-${twin}-mapping.json`), JSON.stringify(lowered, null, 2));
      // Check analysis twins too, so erased package signatures cannot manufacture
      // a diagnostic from a consumer that is impossible under the real types.
      const modeledProgram = ts.createProgram([modeledPath], converted);
      const modeledErrors = ts.getPreEmitDiagnostics(modeledProgram).filter(d => d.category === ts.DiagnosticCategory.Error);
      assert.equal(modeledErrors.length, 0, `${entry.id}/${twin}: analysis twin type errors: ${ts.formatDiagnostics(modeledErrors, {
        getCanonicalFileName: x => x, getCurrentDirectory: () => dir, getNewLine: () => "\n" })}`);
      const projects = {};
      for (const [variant, path] of [["baseline", originalPath], ["modeled", modeledPath]]) {
        const project = join(dir, `${host}-${twin}-${variant}.json`);
        writeFileSync(project, JSON.stringify({ compilerOptions: options, files: [path] }));
        const prior = savedBaseline?.results.find(item => item.id === entry.id)?.observations.find(item => item.host === host && item.twin === twin);
        if (variant === "baseline" && prior) {
          const oldDir = join(dirname(baselinePath), entry.id), stem = `${host}-${twin}`;
          assert.equal(readFileSync(join(oldDir, `${stem}.tsx`), "utf8"), text, "Historical baseline specimen changed");
          assert.deepEqual(read(join(oldDir, `${stem}-baseline.json`)).compilerOptions, options, "Historical baseline options changed");
          projects[variant] = read(join(oldDir, `${stem}-baseline-output.json`));
          projects[variant].historicalSource = baselinePath;
        } else if (variant === "modeled" && prior && process.env.REVIEWED_MODEL_REUSE_MODELED === "1" &&
          readFileSync(join(dirname(baselinePath), entry.id, `${host}-${twin}-modeled.tsx`), "utf8") === lowered.text) {
          const oldDir = join(dirname(baselinePath), entry.id), stem = `${host}-${twin}`;
          assert.deepEqual(read(join(oldDir, `${stem}-modeled.json`)).compilerOptions, options, "Historical modeled options changed");
          projects[variant] = read(join(oldDir, `${stem}-modeled-output.json`));
          projects[variant].historicalSource = baselinePath;
          projects[variant].historicalAnalyzedPath ??= join(oldDir, `${stem}-modeled.tsx`);
        } else projects[variant] = analyze(project, host);
        writeFileSync(join(dir, `${host}-${twin}-${variant}-output.json`), JSON.stringify(projects[variant], null, 2));
      }
      const warnings = projects.modeled.findings.map(f => projectWarning(f, lowered, source, projects.modeled.historicalAnalyzedPath ?? modeledPath)).filter(Boolean);
      const unique = [...new Map(warnings.map(w => [`${w.rule}:${w.location.startByte}`, w])).values()];
      const expectWarning = host === "browser" && twin === "misuse" && entry.expectWarning !== false &&
        !["lifecycle-createIsMounted-module-scope", "media-createMediaQuery-module-scope"].includes(entry.id);
      const observation = { host, twin, expectedWarning: expectWarning, publishedTypingErrors: 0, analysisTwinTypingErrors: 0,
        ...(specialize ? { localCatalogSha256: hash(JSON.stringify(localCatalog, null, 2)), specializationStats: { ...specialize.stats } } : {}),
        preparationMs, baselineMs: projects.baseline.durationMs, modeledMs: projects.modeled.durationMs,
        baselineHistorical: Boolean(projects.baseline.historicalSource), modeledHistorical: Boolean(projects.modeled.historicalSource),
        baseline: projects.baseline.findings.map(({ id, rule, kind, severity }) => ({ id, rule, kind, severity })),
        warnings: unique, sites: lowered.sites, unsupported: lowered.unsupported,
        nativeUnresolvedObligations: projects.modeled.metrics.unresolvedObligations,
        reportsExpected: expectWarning ? unique.some(w => w.rule === entry.rule) : unique.length === 0 };
      if (entry.expectUnsupported && host === "browser") assert(lowered.unsupported.length, `Control did not remain unknown: ${entry.id}`);
      item.observations.push(observation);
    }
  }
  writeFileSync(join(out, "results.json"), JSON.stringify(report, null, 2) + "\n");
  console.log(`${entry.id}: ${item.observations.filter(o => o.reportsExpected).length}/${item.observations.length} expected observations`);
}
const observations = report.results.flatMap(item => item.observations);
const browserMisuses = observations.filter(o => o.host === "browser" && o.expectedWarning);
const negative = observations.filter(o => !o.expectedWarning);
report.summary = { packages: new Set(report.results.map(item => item.package)).size, cases: cases.length, observations: observations.length,
  browserMisuses: browserMisuses.length, modeledDetected: browserMisuses.filter(o => o.reportsExpected).length,
  baselineProvenDetected: report.results.reduce((n, item) => n + item.observations.filter(o => o.expectedWarning && o.baseline.some(f => f.rule === item.rule && f.kind === "violation")).length, 0),
  baselineAnyKindDetected: report.results.reduce((n, item) => n + item.observations.filter(o => o.expectedWarning && o.baseline.some(f => f.rule === item.rule)).length, 0),
  negativeObservations: negative.length, falsePositiveObservations: negative.filter(o => o.warnings.length).length,
  freshBaselineObservations: observations.filter(o => !o.baselineHistorical).length,
  freshModeledObservations: observations.filter(o => !o.modeledHistorical).length,
  failures: report.results.flatMap(item => item.observations.filter(o => !o.reportsExpected).map(o => `${item.id}/${o.host}/${o.twin}`)),
  totalBaselineMs: observations.reduce((n, o) => n + o.baselineMs, 0), totalModeledMs: observations.reduce((n, o) => n + o.modeledMs, 0),
  totalPreparationMs: observations.reduce((n, o) => n + o.preparationMs, 0) };
report.finishedAt = new Date().toISOString();
writeFileSync(join(out, "results.json"), JSON.stringify(report, null, 2) + "\n");
console.log(JSON.stringify(report.summary, null, 2));
if (process.env.REVIEWED_MODEL_EXPLORATORY !== "1") assert.equal(report.summary.failures.length, 0, "Unexpected feedback; inspect the saved results before changing expectations");
else assert.equal(report.summary.falsePositiveObservations, 0, "Exploratory coverage may miss positives; unexpected negative warnings still fail");
