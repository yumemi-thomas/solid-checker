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
for (const [solution, nativeStatus, failure] of [[false, 0], [true, 1],
  ...["config-load", "configure-hook", "early-create", "root-escape", "root-hook", "missing-index", "cleanup-hang", "comparison-native", "unbound-document"].map(failure => [false, 0, failure])]) {
  test(`feedback run drains JSON and diagnostics and exits despite plugin timers (solution=${solution}, failure=${failure ?? "none"})`, () => {
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
    if (failure !== "missing-index") writeFileSync(join(root, "index.html"), '<html><body><script type="module" src="/a/source.ts"></script></body></html>');
    const config = join(root, "vite.config.mjs"); writeFileSync(config, 'export default {};');
    const scenarioPath = join(root, "scenario.json"), viewport = { width: 390, height: 844, isMobile: true, deviceScaleFactor: 2 };
    writeFileSync(scenarioPath, JSON.stringify({ schemaVersion: 1, viewport, steps: [{ action: "wait-for-selector", selector: "body" }] }));
    const binary = join(root, "solid-checker-rust");
    writeFileSync(binary, '#!/bin/sh\n' + (failure === "comparison-native" ? 'case "$2" in */b/*) echo failed >&2; exit 9;; esac\n' : '') + 'echo \'{"findings":[],"feedbackFacts":[]}\'\n' + `exit ${nativeStatus}\n`); chmodSync(binary, 0o755);
    const lifecycle = join(root, "lifecycle.jsonl");
    const log = `import {appendFileSync} from 'node:fs'; const log = row => appendFileSync(${JSON.stringify(lifecycle)}, JSON.stringify(row)+'\\n');`;
    const pkg = (name, code) => {
      const directory = join(root, "node_modules", name); mkdirSync(directory, { recursive: true });
      writeFileSync(join(directory, "package.json"), JSON.stringify({ name, type: "module", exports: { ".": { import: "./index.mjs", default: "./index.mjs" } } }));
      writeFileSync(join(directory, "index.mjs"), code);
    };
    pkg("vite", `${log}
      export const mergeConfig = (app, collector) => ({...app,...collector});
      export async function loadConfigFromFile(_env, file) {
        setInterval(()=>{},1000); log(['config',file]);
        ${failure === "config-load" ? 'throw new Error("timer-bearing config failed");' : ''}
        return {path:file ?? ${JSON.stringify(config)},dependencies:[],config:{root:${JSON.stringify(failure === "root-escape" ? tmpdir() : root)}}};
      }
      export async function createServer(config) {
        log(['server',config.root]); setInterval(()=>{},1000);
        ${failure === "early-create" ? 'throw new Error("Vite failed before exposing a server");' : ''}
        const server = {config:{base:'/',root:${JSON.stringify(failure === "root-hook" ? join(root, "a") : root)}},
          pluginContainer:{resolveId:async()=>({id:${JSON.stringify(join(root, "a/source.ts"))}})},
          moduleGraph:{getModuleById:()=>({}),invalidateModule(){
            for(const plugin of config.plugins) if(plugin.name==='solid-checker-feedback-entry-bootstrap')
              plugin.transform.call({},'export const source=1;',${JSON.stringify(join(root, "a/source.ts"))});
          }},
          middlewares:{use(){}},httpServer:{address:()=>({port:1234})},listen:async()=>{},close:async()=>{
            log(['server-close']); ${failure === "cleanup-hang" ? 'await new Promise(()=>{});' : ''}
          }};
        for (const plugin of config.plugins) await plugin.configureServer?.(server);
        ${failure === "configure-hook" ? 'throw new Error("application configure hook failed");' : ''}
        return server;
      }`);
    pkg("playwright", `${log}
      export const chromium = {launch:async()=>({version:()=> 'test-browser',close:async()=>log(['browser-close']),
        newContext:async options=>{log(['context',options]); return {
          setDefaultTimeout(){},route:async()=>{},close:async()=>log(['context-close']),
          newCDPSession:async()=>({on(){},send:async()=>{}}),newPage:async()=>({on(){},goto:async()=>{},waitForFunction:async()=>{${failure === "unbound-document" ? 'throw Error("binding timeout");' : ''}},
            locator:()=>{log(['scenario-action']);return {waitFor:async()=>{}};},evaluate:async fn=>fn.toString().includes('DOMParser')
              ? [{url:'http://127.0.0.1:1234/a/source.ts',inline:null}]
              : fn.toString().includes('__solidCheckerDocument')
                ? ${failure === "unbound-document" ? "null" : "{id:'test-document',modules:['http://127.0.0.1:1234/a/source.ts'],state:{events:[],stats:{reads:0,retained:0,dropped:0,bytes:0},diagnostics:[]}}"}
                : {events:0,text:''}})
        };}
      })};`);
    pkg("@jridgewell/trace-mapping", 'export class TraceMap {} export function originalPositionFor() {}');
    const run = spawnSync(process.execPath, [cli, "feedback", "run", "--project", project,
      ...(failure === "comparison-native" ? ["--compare-project", join(root, "b/tsconfig.json")] : failure === "root-escape" ? [] : ["--root", root]),
      ...(failure === "root-escape" || failure === "comparison-native" ? [] : ["--config", config]), "--scenario", scenarioPath, "--browser", binary, "--tooling", root, "--native-bin", binary],
    { encoding: "utf8", timeout: 20000, env: { ...process.env, SOLID_CHECKER_TIMINGS: "" } });
    assert.equal(run.error, undefined, run.stderr); assert.equal(run.status, failure ? 2 : nativeStatus, run.stderr);
    const report = JSON.parse(run.stdout);
    if (failure) {
      assert.equal(report.status, failure === "comparison-native" ? "native-failed" : "runtime-failed");
      const expected = { "config-load": /timer-bearing config failed/, "configure-hook": /configure hook failed/,
        "early-create": /no Vite server was exposed for cleanup/, "root-escape": /outside the app tree/,
        "root-hook": /changed the reported root/, "missing-index": /needs an application index.html/,
        "cleanup-hang": /server close timed out/, "comparison-native": /status 9/, "unbound-document": /bootstrap did not bind/ }[failure];
      assert.match(report.failure.message, expected); assert.match(run.stderr, expected);
      const events = readFileSync(lifecycle, "utf8").trim().split("\n").map(JSON.parse);
      assert(events.some(row => row[0] === "browser-close"));
      if (failure === "unbound-document") assert(!events.some(row => row[0] === "scenario-action"));
      if (!["config-load", "early-create", "root-escape"].includes(failure)) assert(events.some(row => row[0] === "server-close"));
      return;
    }
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
}

for (const option of ["--root", "--config"]) test(`feedback refuses ${option} without a value or outside the run command`, () => {
  for (const args of [["run", "--project", "missing", option], ["--project", "missing", option, "/tmp"]]) {
    const run = spawnSync(process.execPath, [cli, "feedback", ...args], { encoding: "utf8" });
    assert.equal(run.status, 2); assert.match(run.stderr, /requires a value|Unknown feedback option/);
  }
});
