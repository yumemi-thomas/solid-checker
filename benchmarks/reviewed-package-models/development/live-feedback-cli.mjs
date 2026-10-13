// Fresh execution through shipped CLI modules. Historical selector/collector
// modules supply no verdicts and are never imported by the production command.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, symlinkSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import cases from "./cases.mjs";
const [outputArg, selectionArg] = process.argv.slice(2), output = resolve(outputArg);
assert(!existsSync(output)); mkdirSync(output, { recursive: true });
const config = JSON.parse(readFileSync("rust/target/development-broad-config-v1.json", "utf8"));
const defaultIds = ["serial", "concurrent"].flatMap(mode => [
  `async-body-return-${mode}-adopted-child-object-deferred`, `async-body-return-${mode}-reaction-read-deferred`,
  `warning-accuracy-fresh-${mode}-named-allocation-open`, `warning-accuracy-fresh-${mode}-identity-control-open`
]);
const ids = selectionArg ? JSON.parse(readFileSync(selectionArg, "utf8")).caseIds : defaultIds;
const selected = ids.map(id => { const row = cases.find(row => row.id === id); assert(row, id); return row; });
const results = [], hash = bytes => `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
const files = ["feedback-browser", "feedback-native-hook", "feedback-read-runtime", "feedback-assertion-selector", "development-feedback"]
  .map(name => resolve(`packages/cli/scripts/${name}.mjs`));
files.push(resolve("packages/cli/bin/solid-checker.mjs"), resolve("packages/cli/bin/launcher.mjs"),
  process.env.SOLID_CHECKER_NATIVE_BIN, process.env.SOLID_TYPEFACTS_BIN);
const implementation = files.map(path => ({ path, sha256: hash(readFileSync(path)) }));
function application(root, challenge, comparison) {
  mkdirSync(join(root, "src"), { recursive: true });
  symlinkSync(join(challenge.install, "node_modules"), join(root, "node_modules"), "dir");
  let source = challenge.source;
  if (comparison) {
    assert.equal(source.split("const read=get;").length, 2, "Only the explicit reviewed captured-read variant is supported");
    source = source.replace("const read=get;", "const captured=get();const read=()=>captured;");
  }
  writeFileSync(join(root, "src/main.tsx"), source);
  writeFileSync(join(root, "src/consumer.ts"), challenge.stages[0].helper);
  for (const [name, code] of Object.entries(challenge.files ?? {})) writeFileSync(join(root, "src", name), code);
  writeFileSync(join(root, "src/setup.ts"), `import {isPending,untrack} from 'solid-js';
    (globalThis as any).__experiment={values:{},errors:[],checkPending:(fn:()=>unknown)=>isPending(()=>untrack(fn))};`);
  writeFileSync(join(root, "src/entry.ts"), `import './setup';import './main';
    const h=(globalThis as any).__experiment;
    const button=document.createElement('button');button.id='update';button.textContent='Update';
    button.onclick=()=>{document.body.dataset.ready='false';h.update();};document.body.append(button);
    function ready(){if(!h.probePending())document.body.dataset.ready='true';requestAnimationFrame(ready);}ready();`);
  writeFileSync(join(root, "index.html"), '<div id="root"></div><script type="module" src="/src/entry.ts"></script>');
  const project = join(root, "tsconfig.json");
  writeFileSync(project, JSON.stringify({ compilerOptions: { target: "ESNext", module: "ESNext", moduleResolution: "bundler",
    jsx: "preserve", jsxImportSource: "@solidjs/web", strict: true, skipLibCheck: true, allowJs: true, noEmit: true }, include: ["src"] }));
  return project;
}
for (const challenge of selected) {
  const start = Date.now(), root = join(output, challenge.id); mkdirSync(root);
  const project = application(join(root, "original"), challenge, false), comparison = application(join(root, "comparison"), challenge, true);
  const stage = challenge.stages[0]; assert.equal(typeof stage.desired, "string");
  const scenario = join(root, "scenario.json");
  writeFileSync(scenario, JSON.stringify({ schemaVersion: 1, steps: [
    { action: "wait-for-text", selector: "#value", text: stage.initial },
    { action: "wait-for-selector", selector: 'body[data-ready="true"]' },
    { action: "click", selector: "#update" },
    { action: "wait-for-selector", selector: 'body[data-ready="true"]' },
    { action: "assert-text", id: "updated-value", selector: "#value", text: stage.desired }
  ] }, null, 2));
  const run = spawnSync(process.execPath, [resolve("packages/cli/bin/solid-checker.mjs"), "feedback", "run", "--project", project,
    "--compare-project", comparison, "--scenario", scenario, "--browser", config.binaries.chromium,
    "--tooling", resolve("rust/target/app-import-metric/apps/helge-dev")], { encoding: "utf8", maxBuffer: 16 * 1024 * 1024, timeout: 90000 });
  writeFileSync(join(root, "stderr.log"), run.stderr ?? "");
  assert([0, 1].includes(run.status), run.error?.message ?? run.stderr);
  const report = JSON.parse(run.stdout); writeFileSync(join(root, "feedback.json"), JSON.stringify(report, null, 2) + "\n");
  assert.equal(report.typingErrorCount, 0); assert.equal(report.comparison.typingErrors, 0);
  assert.deepEqual(report.execution.pageErrors, []); assert.deepEqual(report.comparison.pageErrors, []);
  assert.equal(report.coverage.runtime, "executed-browser"); assert(report.coverage.nativeReadCollection.nativeReader);
  const result = { id: challenge.id, role: stage.role, actual: report.assertions[0].actual, expected: stage.desired,
    comparison: report.comparison.assertions[0].actual, readObservations: report.observations.filter(row => row.kind === "untracked-read").length,
    assertionFailures: report.assertionFailures.length, guidance: report.guidance.notes.length, open: report.guidance.open,
    runtimeCoverage: report.execution.stats, durationMs: Date.now() - start };
  assert.equal(result.actual, stage.afterUpdate);
  assert.equal(result.guidance, stage.role === "target" ? 1 : 0);
  if (stage.role === "target") { assert(result.readObservations > 0); assert.equal(result.assertionFailures, 1); }
  else assert.equal(result.assertionFailures, 0);
  results.push(result); console.log(JSON.stringify(result));
  writeFileSync(join(output, "results.json"), JSON.stringify({ authority: false, certification: false,
    population: "Fresh executions of prior authored consumers with explicit benchmark assertions; no held-out precision claim",
    implementation, results }, null, 2) + "\n");
}
for (const pin of implementation) assert.equal(hash(readFileSync(pin.path)), pin.sha256);
