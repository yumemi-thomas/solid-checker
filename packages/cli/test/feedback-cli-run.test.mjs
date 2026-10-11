import assert from "node:assert/strict";
import { test } from "vitest";
import { spawnSync } from "node:child_process";
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const cli = fileURLToPath(new URL("../bin/solid-checker.mjs", import.meta.url));
// Full CLI orchestration with explicit local tooling doubles: no server,
// browser installation, native build or network is needed. A lingering plugin
// timer reproduces the post-JSON shutdown failure seen in the corpus.
for (const [solution, nativeStatus] of [[false, 0], [true, 1]]) test(`feedback run publishes all JSON, closes resources and exits with a lingering plugin timer (solution=${solution})`, () => {
  const root = realpathSync(mkdtempSync(join(tmpdir(), "solid-feedback-cli-run-")));
  try {
    writeFileSync(join(root, "package.json"), '{"type":"module","workspaces":["a","b"]}');
    for (const name of ["a", "b"]) {
      mkdirSync(join(root, name));
      writeFileSync(join(root, name, "source.ts"), `export const ${name} = 1;`);
      writeFileSync(join(root, name, "tsconfig.json"), JSON.stringify({ compilerOptions: { types: [] }, files: ["source.ts"] }));
    }
    const project = solution ? join(root, "tsconfig.json") : join(root, "a/tsconfig.json");
    if (solution) writeFileSync(project, JSON.stringify({ files: [], references: [{ path: "./a" }, { path: "./b" }] }));
    writeFileSync(join(root, "index.html"), '<html><body></body></html>');
    const config = join(root, "vite.config.mjs"); writeFileSync(config, 'export default {};');
    const scenarioPath = join(root, "scenario.json"), viewport = { width: 390, height: 844, isMobile: true, deviceScaleFactor: 2 };
    writeFileSync(scenarioPath, JSON.stringify({ schemaVersion: 1, viewport, steps: [{ action: "wait-for-selector", selector: "body" }] }));
    const binary = join(root, "solid-checker-rust");
    writeFileSync(binary, '#!/bin/sh\necho \'{"findings":[],"feedbackFacts":[]}\'\n' + `exit ${nativeStatus}\n`); chmodSync(binary, 0o755);
    const lifecycle = join(root, "lifecycle.jsonl");
    const log = `import {appendFileSync} from 'node:fs'; const log = row => appendFileSync(${JSON.stringify(lifecycle)}, JSON.stringify(row)+'\\n');`;
    const pkg = (name, code) => {
      const directory = join(root, "node_modules", name); mkdirSync(directory, { recursive: true });
      writeFileSync(join(directory, "package.json"), JSON.stringify({ name, type: "module", exports: { ".": { import: "./index.mjs", default: "./index.mjs" } } }));
      writeFileSync(join(directory, "index.mjs"), code);
    };
    pkg("vite", `${log}
      export const mergeConfig = (app, collector) => ({...app,...collector});
      export async function loadConfigFromFile(_env, file) { log(['config',file]); return {path:file,dependencies:[],config:{}}; }
      export async function createServer(config) {
        log(['server',config.root]); setInterval(()=>{},1000);
        return {config:{base:'/'},httpServer:{address:()=>({port:1234})},listen:async()=>{},close:async()=>log(['server-close'])};
      }`);
    pkg("playwright", `${log}
      export const chromium = {launch:async()=>({version:()=> 'test-browser',close:async()=>log(['browser-close']),
        newContext:async options=>{log(['context',options]); return {
          setDefaultTimeout(){},route:async()=>{},close:async()=>log(['context-close']),
          newCDPSession:async()=>({on(){},send:async()=>{}}),newPage:async()=>({on(){},goto:async()=>{},
            locator:()=>({waitFor:async()=>{}}),evaluate:async fn=>fn.toString().includes('__solidCheckerDocument') ? null : {events:0,text:''}})
        };}
      })};`);
    pkg("@jridgewell/trace-mapping", 'export class TraceMap {} export function originalPositionFor() {}');
    const run = spawnSync(process.execPath, [cli, "feedback", "run", "--project", project,
      "--root", root, "--config", config, "--scenario", scenarioPath, "--browser", binary, "--tooling", root, "--native-bin", binary],
    { encoding: "utf8", timeout: 20000, env: { ...process.env, SOLID_CHECKER_TIMINGS: "" } });
    assert.equal(run.error, undefined, run.stderr); assert.equal(run.status, nativeStatus, run.stderr);
    const report = JSON.parse(run.stdout);
    assert.equal(report.nativeExitCode, nativeStatus);
    assert.equal(report.execution.failure, null); assert.deepEqual(report.execution.viewport, viewport);
    assert.equal(report.execution.configuration.root, root); assert.equal(report.execution.configuration.path, config);
    assert.deepEqual(report.execution.configuration.options, { root, config });
    assert(report.execution.runtimeInputs.some(pin => pin.path === config && pin.sha256.startsWith("sha256:")));
    const events = readFileSync(lifecycle, "utf8").trim().split("\n").map(JSON.parse);
    assert.deepEqual(events.filter(row => row[0].endsWith("-close")), [["context-close"], ["server-close"], ["browser-close"]]);
    assert.equal(events.filter(row => row[0] === "server").length, 1);
    assert.deepEqual(events.find(row => row[0] === "context")[1], { viewport: { width: 390, height: 844 }, isMobile: true, deviceScaleFactor: 2 });
    if (solution) assert.deepEqual(report.solution.referencedProjects, [join(root, "a/tsconfig.json"), join(root, "b/tsconfig.json")]);
  } finally { rmSync(root, { recursive: true, force: true }); }
});

for (const option of ["--root", "--config"]) test(`feedback refuses ${option} without a value or outside the run command`, () => {
  for (const args of [["run", "--project", "missing", option], ["--project", "missing", option, "/tmp"]]) {
    const run = spawnSync(process.execPath, [cli, "feedback", ...args], { encoding: "utf8" });
    assert.equal(run.status, 2); assert.match(run.stderr, /requires a value|Unknown feedback option/);
  }
});
