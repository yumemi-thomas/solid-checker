import assert from "node:assert/strict";
import { test } from "vitest";
import { spawn } from "node:child_process";
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const cli = fileURLToPath(new URL("../bin/solid-checker.mjs", import.meta.url));

// `feedback batch` with local tooling doubles (as feedback-cli-run.test.mjs): no server, browser installation, native
// build or network. The doubles log their lifecycle, so a test reads which resources each job had and which it shared.
function fixture() {
  const root = realpathSync(mkdtempSync(join(tmpdir(), "solid-feedback-cli-batch-")));
  writeFileSync(join(root, "package.json"), '{"type":"module"}');
  mkdirSync(join(root, "a"));
  writeFileSync(join(root, "a/source.ts"), "export const a = 1;");
  writeFileSync(join(root, "tsconfig.json"), JSON.stringify({ compilerOptions: { types: [] }, files: ["a/source.ts"] }));
  writeFileSync(join(root, "index.html"), '<html><body><script type="module" src="/a/source.ts"></script></body></html>');
  writeFileSync(join(root, "scenario.json"), JSON.stringify({ schemaVersion: 1, steps: [{ action: "wait-for-selector", selector: "body" }] }));
  const binary = join(root, "solid-checker-rust");
  writeFileSync(binary, "#!/bin/sh\necho '{\"findings\":[],\"feedbackFacts\":[]}'\n"); chmodSync(binary, 0o755);
  const lifecycle = join(root, "lifecycle.jsonl");
  const marker = name => JSON.stringify(join(root, name));
  const log = `import {appendFileSync, existsSync} from 'node:fs'; const log = row => appendFileSync(${JSON.stringify(lifecycle)}, JSON.stringify(row)+'\\n');`;
  const pkg = (name, code) => {
    const directory = join(root, "node_modules", name); mkdirSync(directory, { recursive: true });
    writeFileSync(join(directory, "package.json"), JSON.stringify({ name, type: "module", exports: { ".": { import: "./index.mjs", default: "./index.mjs" } } }));
    writeFileSync(join(directory, "index.mjs"), code);
  };
  pkg("vite", `${log}
    export const mergeConfig = (app, collector) => ({...app,...collector});
    export async function loadConfigFromFile(_env, file) {
      if (!existsSync(${marker("app-config")})) return null;
      return {path:${marker("app-config")},dependencies:[],config:{}};
    }
    export async function createServer(config) {
      // What a fresh process has: Vite sets NODE_ENV on the process, so the double does too.
      log(['server', process.env.SC_BATCH_DOUBLE ?? null]); process.env.SC_BATCH_DOUBLE = 'set by an earlier server';
      if (existsSync(${marker("refuse-server")})) throw new Error("server double refused");
      const server = {config:{base:'/',root:${JSON.stringify(root)}},
        pluginContainer:{resolveId:async id=>({id:id==='virtual:solid-checker-feedback-entry'?'\\0solid-checker-feedback-entry':${JSON.stringify(join(root, "a/source.ts"))}})},
        transformRequest:async()=>({code:'import "/@id/__x00__solid-checker-feedback-entry";\\nexport const source=1;'}),
        moduleGraph:{getModuleByUrl:async()=>({id:${JSON.stringify(join(root, "a/source.ts"))}}),getModuleById:()=>({}),invalidateModule(){
          for(const plugin of config.plugins) if(plugin.name==='solid-checker-feedback-entry-bootstrap')
            plugin.transform.call({},'export const source=1;',${JSON.stringify(join(root, "a/source.ts"))});
        }},
        middlewares:{use(){}},httpServer:{address:()=>({port:1234})},listen:async()=>{},close:async()=>log(['server-close'])};
      for (const plugin of config.plugins) await plugin.configureServer?.(server);
      globalThis.__feedbackServer=server;
      return server;
    }`);
  pkg("@solidjs/vite-plugin", "export default () => ({ name: 'solid-double' });");
  pkg("playwright", `${log}
    import {EventEmitter} from 'node:events';
    const open = new Set();
    function session() {
      const cdp=new EventEmitter(); cdp.send=async method=>method==='Page.getFrameTree'
        ? {frameTree:{frame:{id:'top'}}} : method==='Network.getResponseBody'
        ? {body:'<script type="module" src="/a/source.ts"></script>',base64Encoded:false} : {};
      return cdp;
    }
    export const chromium = {launch:async()=>{ log(['launch']); return {version:()=> 'test-browser',on(){},contexts:()=>[...open],close:async()=>log(['browser-close']),
      newContext:async()=>{log(['context']); const cdp=session(); const context={
        setDefaultTimeout(){},route:async()=>{},close:async()=>{log(['context-close']); if (!existsSync(${marker("keep-context")})) open.delete(context);},
        newCDPSession:async()=>cdp,newPage:async()=>({on(){},goto:async()=>{
          await globalThis.__feedbackServer.transformRequest('/a/source.ts');
          cdp.emit('Page.frameNavigated',{frame:{id:'top',loaderId:'loader',url:'http://127.0.0.1:1234/'}});
          cdp.emit('Runtime.executionContextCreated',{context:{id:1,auxData:{frameId:'top',isDefault:true}}});
          cdp.emit('Runtime.bindingCalled',{name:'__solidCheckerFlush',executionContextId:1,payload:JSON.stringify({phase:'start',id:'test-document'})});
          cdp.emit('Network.responseReceived',{type:'Document',frameId:'top',loaderId:'loader',requestId:'request',response:{url:'http://127.0.0.1:1234/',status:200,mimeType:'text/html'}});
          cdp.emit('Network.loadingFinished',{requestId:'request',encodedDataLength:64});
        },waitForFunction:async()=>{},
          locator:()=>{log(['scenario-action']);return {waitFor:async()=>{}};},evaluate:async fn=>fn.toString().includes('DOMParser')
            ? [{url:'http://127.0.0.1:1234/a/source.ts',inline:null}]
            : fn.toString().includes('__solidCheckerDocument')
              ? {id:'test-document',modules:['http://127.0.0.1:1234/a/source.ts'],state:{events:[],stats:{reads:0,retained:0,dropped:0,bytes:0},diagnostics:[]}}
              : {events:0,text:''}})
      }; open.add(context); return context;}}; }};`);
  pkg("@jridgewell/trace-mapping", "export class TraceMap {} export function originalPositionFor() {}");
  // The browser binary is only hashed and stamped: the native double stands in for it.
  const events = () => existsSync(lifecycle) ? readFileSync(lifecycle, "utf8").trim().split("\n").map(row => JSON.parse(row)) : [];
  return { root, binary, events, touch: name => writeFileSync(join(root, name), "") };
}

// Runs a batch, sending each job only after the previous answer, so `between(index)` can change the fixture in between.
function batch({ root, binary }, count, between = () => {}) {
  return new Promise((done, fail) => {
    const child = spawn(process.execPath, [cli, "feedback", "batch", "--browser", binary, "--tooling", root, "--native-bin", binary],
      { cwd: root, env: { ...process.env, SOLID_CHECKER_TIMINGS: "", SC_BATCH_DOUBLE: "" }, stdio: ["pipe", "pipe", "pipe"] });
    const answers = [];
    let stdout = "", stderr = "", sent = 0;
    const send = () => {
      if (sent === count) { child.stdin.end(); return; }
      between(sent);
      child.stdin.write(JSON.stringify({ project: join(root, "tsconfig.json"), scenario: join(root, "scenario.json"), report: join(root, `report-${sent}.json`) }) + "\n");
      sent++;
    };
    const timer = setTimeout(() => { child.kill("SIGKILL"); fail(new Error(`batch timed out: ${stderr}`)); }, 30000);
    child.stdout.on("data", chunk => {
      stdout += chunk;
      for (let newline; (newline = stdout.indexOf("\n")) >= 0;) {
        const row = JSON.parse(stdout.slice(0, newline)); stdout = stdout.slice(newline + 1);
        answers.push(row);
        if (row.report && !row.retire) send();
      }
    });
    child.stderr.on("data", chunk => { stderr += chunk; });
    child.stdin.on("error", () => {});
    child.on("close", status => { clearTimeout(timer); done({ status, stderr, answers }); });
    send();
  });
}

test("feedback batch gives each job its own server and context, one browser, and a fresh environment", async () => {
  const run = fixture();
  try {
    const { status, stderr, answers } = await batch(run, 3);
    assert.equal(status, 0, stderr);
    assert.deepEqual(answers.map(row => "retire" in row ? row.retire : row.closed), [null, null, null, true]);
    for (const index of [0, 1, 2]) {
      const report = JSON.parse(readFileSync(join(run.root, `report-${index}.json`), "utf8"));
      assert.equal(report.execution.failure, null); assert.equal(report.execution.configuration.kind, "collector-default");
      assert.equal(answers[index].exitCode, report.nativeExitCode);
    }
    const events = run.events();
    assert.equal(events.filter(row => row[0] === "launch").length, 1);
    // Every server starts from the environment the batch opened with, as a separate `feedback run` would.
    assert.deepEqual(events.filter(row => row[0] === "server"), [["server", ""], ["server", ""], ["server", ""]]);
    assert.deepEqual(events.filter(row => row[0].endsWith("-close")).map(row => row[0]),
      ["context-close", "server-close", "context-close", "server-close", "context-close", "server-close", "browser-close"]);
  } finally { rmSync(run.root, { recursive: true, force: true }); }
});

for (const [name, change, expected] of [
  ["a failed run", run => run.touch("refuse-server"), /the run failed: server double refused/],
  ["an application config", run => run.touch("app-config"), /application Vite config/],
  ["a context that outlived its run", run => run.touch("keep-context"), /outlived its run/],
  ["a browser binary that changed", run => writeFileSync(run.binary, readFileSync(run.binary, "utf8") + "\n"), /the run failed: Browser changed during execution/],
]) test(`feedback batch retires after ${name} and takes no later job`, async () => {
  const run = fixture();
  try {
    const { status, stderr, answers } = await batch(run, 3, index => index === 1 && change(run));
    const retired = answers[1];
    assert.match(String(retired.retire), expected);
    assert.equal(answers[0].retire, null);
    assert.equal(existsSync(join(run.root, "report-2.json")), false);
    if (name === "a failed run") {
      assert.equal(retired.exitCode, 2);
      assert.equal(JSON.parse(readFileSync(retired.report, "utf8")).status, "runtime-failed");
    }
    // The close hashes the browser binary again: a changed one fails the close too, and the caller drops the batch's runs.
    if (name === "a browser binary that changed") { assert.equal(status, 2); assert.equal(answers.length, 2); assert.match(stderr, /Browser changed/); }
    else { assert.equal(status, 0, stderr); assert.deepEqual(answers[2], { closed: true }); assert.equal(answers.length, 3); }
    assert.equal(run.events().filter(row => row[0] === "launch").length, 1);
  } finally { rmSync(run.root, { recursive: true, force: true }); }
});
