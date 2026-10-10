import assert from "node:assert/strict";
import { existsSync, mkdirSync, symlinkSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { spawnSync } from "node:child_process";
import { authenticateModel, hash, read } from "./catalog.mjs";
import { installedCatalog } from "./demand-models.mjs";
import { ts } from "./lower.mjs";
import { oracleCompilerOptions } from "../../scripts/tsc-oracle.mjs";

const [runArgument, outArgument, filter = ""] = process.argv.slice(2);
assert(outArgument, "Usage: node runtime-feedback-experiment.mjs <retained-run.json> <fresh-output-directory>");
const run = read(resolve(runArgument)), out = resolve(outArgument); assert(!existsSync(out)); mkdirSync(out, { recursive: true });
const bridge = new URL("./runtime-feedback.mjs", import.meta.url).pathname;
const cases = [
  { id: "timer-literal-unowned", package: "timer", setup: `import { createTimer } from "@solid-primitives/timer";`,
    body: `createTimer(() => {}, 1, setTimeout);`, codes: ["NO_OWNER_CLEANUP"] },
  { id: "timer-function-unowned", package: "timer", setup: `import { createTimer } from "@solid-primitives/timer";`,
    body: `createTimer(() => {}, () => 1, setTimeout);`, codes: ["NO_OWNER_EFFECT"] },
  { id: "timer-owned", package: "timer", setup: `import { createTimer } from "@solid-primitives/timer";`,
    body: `const dispose = createRoot(dispose => { createTimer(() => {}, 1, setTimeout); return dispose; }); dispose();`, codes: [] },
  { id: "pagination-wrapper-eager", package: "pagination", setup: `import { createPagination } from "@solid-primitives/pagination";
function helper() { return createPagination({ pages: 10 }); }`,
    body: `const dispose = createRoot(dispose => { createComponent(function App() { const [, page] = helper(); page(); return null; }, {}); return dispose; }); dispose();`, codes: ["STRICT_READ_UNTRACKED", "STRICT_READ_UNTRACKED"] },
  { id: "pagination-wrapper-tracked", package: "pagination", setup: `import { createPagination } from "@solid-primitives/pagination";
function helper() { return createPagination({ pages: 10 }); }`,
    body: `const dispose = createRoot(dispose => { createComponent(function App() { const [, page] = helper(); createEffect(() => page(), () => {}); return null; }, {}); return dispose; }); flush(); dispose();`, codes: ["STRICT_READ_UNTRACKED"] },
  { id: "memo-tracked-write", package: "memo", setup: `import { createLazyMemo } from "@solid-primitives/memo";`,
    body: `const dispose = createRoot(dispose => { createComponent(function App() { const [count, setCount] = createSignal(1);
const read = createLazyMemo(() => { setCount(2); return count(); }); createEffect(() => read(), () => {}); return null; }, {}); return dispose; }); flush(); dispose();`, codes: ["REACTIVE_WRITE_IN_OWNED_SCOPE"] },
  { id: "memo-tracked-read", package: "memo", setup: `import { createLazyMemo } from "@solid-primitives/memo";`,
    body: `const dispose = createRoot(dispose => { createComponent(function App() { const [count] = createSignal(1);
const read = createLazyMemo(() => count()); createEffect(() => read(), () => {}); return null; }, {}); return dispose; }); flush(); dispose();`, codes: [] },
];
const report = { authority: false, runtime: "2.0.0-rc.9", host: "browser/development module resolution in Node", results: [], finishedAt: null };
for (const entry of cases.filter(entry => !filter || filter.split(",").includes(entry.id))) {
  const dir = join(out, entry.id); mkdirSync(dir);
  const row = run.results.find(row => row.package === `@solid-primitives/${entry.package}`);
  assert(row, "Missing retained install"); symlinkSync(join(row.retainedArtifacts.projectDir, "node_modules"), join(dir, "node_modules"), "dir");
  const catalog = installedCatalog(dir, [{ package: row.package, exports: [] }]);
  assert.equal(catalog.packages.filter(item => item.error).length, 0); authenticateModel(catalog.models[0], dir);
  // The exact executable specimen is type-checked against published types.
  // The observer file is JavaScript; allowJs admits it without weakening the
  // external package's declarations or the strict consumer signatures.
  const code = `import { OBSERVE, createRoot, createComponent, createEffect, createSignal, flush } from "solid-js";
import { runtimeFeedback } from ${JSON.stringify(bridge)};
${entry.setup}
const observer = runtimeFeedback(OBSERVE, { appRoot: ${JSON.stringify(dir)} });
${entry.body}
observer.stop(); console.log(JSON.stringify(observer.feedback));`;
  const sourcePath = join(dir, "App.ts"); writeFileSync(sourcePath, code);
  const options = ts.convertCompilerOptionsFromJson({ ...oracleCompilerOptions("v2", true, { customConditions: ["browser", "development"] }), allowJs: true }, dir).options;
  const program = ts.createProgram([sourcePath], options), errors = ts.getPreEmitDiagnostics(program).filter(d => d.category === ts.DiagnosticCategory.Error);
  assert.equal(errors.length, 0, ts.formatDiagnostics(errors, { getCanonicalFileName: x => x, getCurrentDirectory: () => dir, getNewLine: () => "\n" }));
  const path = join(dir, "App.mjs"), executable = ts.transpileModule(code, { compilerOptions: { target: ts.ScriptTarget.ESNext, module: ts.ModuleKind.ESNext } }).outputText;
  writeFileSync(path, executable);
  const response = spawnSync(process.execPath, ["--conditions=browser", "--conditions=development", path], { cwd: dir, encoding: "utf8", timeout: 10000 });
  const item = { id: entry.id, package: row.package, version: row.version, pins: catalog.models[0].pins,
    publishedTypingErrors: 0, executableSha256: hash(executable), stdout: response.stdout, stderr: response.stderr, exitCode: response.status };
  report.results.push(item); writeFileSync(join(out, "results.json"), JSON.stringify(report, null, 2) + "\n");
  assert.equal(response.status, 0, response.stderr);
  const feedback = JSON.parse(response.stdout); item.feedback = feedback;
  assert.deepEqual(feedback.map(item => item.code), entry.codes, entry.id);
  for (const item of feedback) {
    assert.equal(item.location?.path, pathToFileURL(path).href, "Diagnostic must locate the real consumer frame");
    assert(item.location.line > 0); assert.equal(item.certification, false);
  }
  if (entry.codes.length && entry.id.startsWith("timer-"))
    assert(feedback.some(item => item.frames.some(frame => frame.path.includes("/node_modules/@solid-primitives/timer/"))), "Timer setup frame missing");
  if (entry.id === "pagination-wrapper-eager") {
    assert(feedback[0].frames.some(frame => frame.path.includes("/node_modules/@solid-primitives/pagination/")), "Internal package read frame missing");
    assert(!feedback[1].frames.some(frame => frame.path.includes("/node_modules/@solid-primitives/pagination/")), "Returned accessor read need not retain its factory frame");
  }
  console.log(`${entry.id}: ${feedback.length} runtime observations with consumer locations`);
}
report.finishedAt = new Date().toISOString(); writeFileSync(join(out, "results.json"), JSON.stringify(report, null, 2) + "\n");
