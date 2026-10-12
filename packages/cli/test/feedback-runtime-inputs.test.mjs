import assert from "node:assert/strict";
import { test } from "vitest";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createHash } from "node:crypto";
import { SourceMap } from "node:module";
import { bootstrapFeedbackEntry, completeFeedbackEntryOutput, feedbackEntryBootstrapPlugin, normalizeFeedbackEntryRequest, pinFeedbackEntryOutput, resolveFeedbackEntries, selectedFeedbackEntry, validateFeedbackDocument, validateFeedbackServedEntries } from "../scripts/feedback-entry-bootstrap.mjs";
import { captureFeedbackOptimizer, configureFeedbackOptimizer, createFeedbackOptimizerObserver, resolveFeedbackRuntime } from "../scripts/feedback-optimizer.mjs";
import { feedbackReadPlugin, mergeDocumentStates, mergeFeedbackServerConfiguration } from "../scripts/feedback-browser.mjs";
import { createServer, mergeConfig } from "vite";

test("optimizer retains CommonJS includes and discovery while excluding all Solid runtime subpaths", () => {
  const options = () => ({ noDiscovery: true, include: ["solid-js", "solid-js/store", "@solidjs/signals", "style-to-js", "a > @solidjs/web", "solid-js > cjs-helper"], exclude: ["other"] });
  const config = { optimizeDeps: options(), environments: { client: { optimizeDeps: options() } } };
  configureFeedbackOptimizer(config);
  for (const row of [config.optimizeDeps, config.environments.client.optimizeDeps]) {
    assert.equal(row.noDiscovery, false);
    assert.deepEqual(row.include, ["style-to-js", "solid-js > cjs-helper"]);
    assert.deepEqual(row.exclude, ["other", "solid-js", "@solidjs/signals", "@solidjs/web"]);
  }
});

test("captured /@id/ entries resolve exactly, including encoded null IDs and HTML base URLs", async () => {
  const ids = [], server = { config: { base: "/app/" }, environments: { client: { pluginContainer: {
    resolveId: async id => { ids.push(id); return { id: id === "virtual:app.tsx" ? "\0actual-app" : id }; }
  } } } };
  const result = await resolveFeedbackEntries([
    { url: "http://app.test/app/@vite/client" }, { url: "http://app.test/app/@id/virtual:app.tsx" },
    { url: "http://app.test/app/@id/__x00__exact" }, { url: "http://app.test/app/nested/entry.js?mode=client" }
  ], "http://app.test/app/", server);
  assert.deepEqual(ids, ["virtual:app.tsx", "\0exact", "/nested/entry.js?mode=client"]);
  assert.equal(result[0].id, "\0actual-app");
  await assert.rejects(resolveFeedbackEntries([{ url: "https://foreign.test/main.js" }], "http://app.test/app/", server), /foreign/);
  await assert.rejects(resolveFeedbackEntries([{ url: null, inline: "run()" }], "http://app.test/app/", server), /inline/);
  await assert.rejects(resolveFeedbackEntries([], "http://app.test/app/", server), /module entry/);
  await assert.rejects(resolveFeedbackEntries([{ url: "http://app.test/main.js" }], "http://app.test/", {
    config: { base: "/" }, pluginContainer: { resolveId: async () => null }
  }), /cannot resolve/);
});

test("entry prefix preserves bytes and exact line/column attribution", () => {
  const code = 'const value = "日本語";\nthrow new Error(value);', result = bootstrapFeedbackEntry(code, "/app.js", "virtual:bootstrap");
  assert.equal(result.code, 'import "virtual:bootstrap";\n' + code);
  const map = new SourceMap(result.map);
  assert.deepEqual(map.findEntry(2, 17), { generatedLine: 2, generatedColumn: 17, originalSource: "/app.js", originalLine: 1, originalColumn: 17, name: undefined });
  const completed = completeFeedbackEntryOutput(result, "/app.js", "\0bootstrap", "/");
  assert.equal(completed.code, 'import "/@id/__x00__bootstrap";\n' + result.code);
  assert.deepEqual(new SourceMap(completed.map).findEntry(3, 17), { ...map.findEntry(2, 17), generatedLine: 3 });
  assert.equal(completeFeedbackEntryOutput(completed, "/app.js", "\0bootstrap", "/"), completed);
});

test("physical and query-sensitive virtual entry selection matches Vite's HTTP transform IDs", async () => {
  const root = realpathSync(mkdtempSync(join(tmpdir(), "solid-feedback-entry-request-")));
  let server;
  try {
    writeFileSync(join(root, "entry.js"), "export const physical = true;");
    const transformed = [], entries = new Set();
    server = await createServer({ root, base: "/app/", configFile: false, logLevel: "silent",
      server: { host: "127.0.0.1", port: 0 }, optimizeDeps: { noDiscovery: true }, plugins: [{
        name: "query-sensitive-entry",
        resolveId(id) { if (id.startsWith("virtual:entry?")) return "\0" + id;
          if (id.startsWith("\0virtual:entry?")) return id; if (id === "virtual:bootstrap") return "\0bootstrap"; },
        load(id) { if (id.startsWith("\0")) return "export const virtual = true;"; },
        transform(_code, id) { transformed.push(id); }
      }, feedbackEntryBootstrapPlugin(entries, "virtual:bootstrap")] });
    await server.listen();
    const documentUrl = `http://127.0.0.1:${server.httpServer.address().port}/app/`;
    for (const path of ["entry.js", "@id/virtual:entry", "@id/__x00__virtual:entry"])
      for (const encoded of ["%26", "%2F", "%3F", "%23"]) {
        const url = new URL(`${path}?label=a${encoded}b&t=1234567890123&import=`, documentUrl).href;
        const [selected] = await resolveFeedbackEntries([{ url }], documentUrl, server);
        entries.add(selected.id);
        const response = await fetch(url, { headers: { "sec-fetch-dest": "script" } });
        const code = await response.text();
        assert.equal(response.status, 200, code);
        assert(transformed.includes(selected.id), `Selected ID did not reach the actual HTTP transform: ${selected.id}`);
        assert(selected.id.endsWith(`?label=a${encoded}b`));
        pinFeedbackEntryOutput({ code }, selected.id, "\0bootstrap", "/app/");
      }
    assert.equal(normalizeFeedbackEntryRequest("/app/@id/__x00__entry?t=1234567890123", "/app/"), "\0entry");
  } finally { if (server) await server.close(); rmSync(root, { recursive: true, force: true }); }
});

test("bootstrap transforms only selected client entries, leaving virtual helpers, CSS and SSR alone", () => {
  const entries = new Set(["\0entry.tsx", "/entry.js"]), observed = [];
  const plugin = feedbackEntryBootstrapPlugin(entries, "virtual:bootstrap", id => observed.push(id));
  for (const id of entries) assert(plugin.transform.call({ environment: { name: "client" } }, "run();", id).code.startsWith('import "virtual:bootstrap"'));
  for (const id of ["\0helper.js", "\0helper.css", "/other.js"]) assert.equal(plugin.transform.call({}, "untouched", id), null);
  assert.equal(plugin.transform.call({}, "run();", "/entry.js", { ssr: true }), null);
  assert.equal(plugin.transform.call({ environment: { name: "ssr" } }, "run();", "/entry.js"), null);
  assert.deepEqual(observed, [...entries]);
  const reader = feedbackReadPlugin({ snapshot: {}, runtimeInputs: new Map(), coverage: {} });
  assert.equal(reader.transformIndexHtml()[0].attrs.src, "/@id/virtual:solid-checker-feedback-entry");
  assert.equal(reader.transformIndexHtml("", { server: { config: { base: "/app/" } } })[0].attrs.src,
    "/app/@id/virtual:solid-checker-feedback-entry");
});

test("middleware cannot replace or add module entries while retaining a successful binding", () => {
  const entries = [{ url: "http://app.test/entry.js" }], helper = "http://app.test/@vite/client";
  assert.doesNotThrow(() => validateFeedbackServedEntries([helper, entries[0].url], entries, [helper]));
  assert.throws(() => validateFeedbackServedEntries([helper, entries[0].url, "http://app.test/other.js"], entries, [helper]), /unselected module entry/);
  assert.throws(() => validateFeedbackServedEntries([helper], entries, [helper]), /replaced a selected module entry/);
  assert.throws(() => validateFeedbackServedEntries([entries[0].url, null], entries, [helper]), /unselected module entry/);
});

for (const late of [false, true]) test(`entry bootstrap precedes application post imports and Vite helpers (hook-order-post=${late})`, async () => {
  const plugin = feedbackReadPlugin({ snapshot: {}, runtimeInputs: new Map(), coverage: {} });
  const data = code => "data:text/javascript;base64," + Buffer.from(code).toString("base64");
  const messages = [], runtime = data(`export const reads=globalThis.__solidCheckerReads={events:[],diagnostics:[],stats:{reads:0,retained:0,dropped:0,bytes:0},scopes:{rows:[],dropped:0},diagnostic(e){this.diagnostics.push(e)}}; // late=${late}`);
  const solid = data(`export const OBSERVE={diagnostics:{subscribe(callback){globalThis.__feedbackEmit=callback}}}; // late=${late}`);
  const bootstrap = data(plugin.load("\0solid-checker-feedback-entry").replace(JSON.stringify("/@fs" + new URL("../scripts/feedback-read-runtime.mjs", import.meta.url).pathname), JSON.stringify(runtime))
    .replace("'solid-js'", JSON.stringify(solid)));
  const dependency = data(`if(!globalThis.__solidCheckerDocument)throw Error('entry ran before bootstrap');
    globalThis.__solidCheckerReads.events.push({kind:'untracked-read'});globalThis.__feedbackEmit({code:'STRICT_READ_UNTRACKED'}); // late=${late}`);
  const previous = Object.fromEntries(["addEventListener", "__solidCheckerFlush", "__solidCheckerDocument", "__solidCheckerReads", "__feedbackEmit"].map(key => [key, globalThis[key]]));
  const root = realpathSync(mkdtempSync(join(tmpdir(), "solid-feedback-entry-order-")));
  let server;
  try {
    globalThis.addEventListener = () => {};
    globalThis.__solidCheckerFlush = payload => messages.push(JSON.parse(payload));
    writeFileSync(join(root, "entry.js"), "globalThis.__feedbackEntryExecuted = true; if (import.meta.hot) {};");
    const entryId = join(root, "entry.js"), pins = [];
    const prepend = (code, id) => { if (id === entryId) return { code: `import ${JSON.stringify(dependency)};\n${code}`, map: null }; };
    const application = { root, configFile: false, logLevel: "silent", optimizeDeps: { noDiscovery: true }, plugins: [{
      name: "application-post-startup", enforce: "post",
      transform: late ? { order: "post", handler: prepend } : prepend
    }] };
    const collector = { plugins: [{ name: "bootstrap-loader", resolveId(id) { if (id === "virtual:bootstrap") return "\0bootstrap"; },
      load(id) { if (id === "\0bootstrap") return "export {};"; } },
      feedbackEntryBootstrapPlugin(new Set([entryId]), "virtual:bootstrap", (_id, code) => pins.push(code))] };
    server = await createServer(mergeFeedbackServerConfiguration({ mergeConfig }, application, collector));
    const transformedEntry = completeFeedbackEntryOutput(await server.transformRequest("/entry.js"), entryId, "\0bootstrap", "/");
    pinFeedbackEntryOutput(transformedEntry, entryId, "\0bootstrap", "/");
    if (!late) assert(pins[0].startsWith(`import ${JSON.stringify(dependency)};`));
    assert(transformedEntry.code.includes(`import ${JSON.stringify(dependency)};`));
    const client = data("export const createHotContext = () => ({});");
    await import(data(transformedEntry.code.replaceAll(JSON.stringify("/@id/__x00__bootstrap"), JSON.stringify(bootstrap))
      .replace(JSON.stringify("/@vite/client"), JSON.stringify(client)).replaceAll("import.meta.hot", "globalThis.__feedbackHot")));
    assert.equal(globalThis.__feedbackEntryExecuted, true);
    const final = { id: globalThis.__solidCheckerDocument, state: globalThis.__solidCheckerReads };
    validateFeedbackDocument(final);
    const result = mergeDocumentStates(messages, final);
    assert.deepEqual(result.documents, { loaded: 1, reported: 1, lost: 0 });
    assert.equal(result.events.length, 1); assert.equal(result.diagnostics[0].code, "STRICT_READ_UNTRACKED");
    assert.throws(() => validateFeedbackDocument(null), /bootstrap did not bind/);
    assert.throws(() => pinFeedbackEntryOutput({ code: 'import "startup";\n' + transformedEntry.code }, entryId, "\0bootstrap", "/"), /does not start/);
  } finally {
    if (server) await server.close(); rmSync(root, { recursive: true, force: true }); delete globalThis.__feedbackEntryExecuted;
    delete globalThis.__feedbackHot;
    for (const [key, value] of Object.entries(previous)) { if (value === undefined) delete globalThis[key]; else globalThis[key] = value; }
  }
});

test("optimized metadata and bundles are pinned; runtime inlining, substitution and missing maps refuse", () => {
  const root = realpathSync(mkdtempSync(join(tmpdir(), "solid-feedback-optimizer-")));
  try {
    const cache = join(root, "cache"), deps = join(cache, "deps"), pkg = join(root, "node_modules/cjs");
    mkdirSync(deps, { recursive: true }); mkdirSync(pkg, { recursive: true });
    writeFileSync(join(pkg, "package.json"), '{"name":"cjs"}');
    const src = join(pkg, "index.js"), file = join(deps, "cjs.js");
    writeFileSync(src, "module.exports=1;"); writeFileSync(file, "export default 1;");
    const metadata = { hash: "hash", lockfileHash: "lock", configHash: "config", browserHash: "browser", optimized: { cjs: { src, file: "cjs.js", needsInterop: true } }, chunks: {} };
    writeFileSync(join(deps, "_metadata.json"), JSON.stringify(metadata));
    const map = { version: 3, sources: [src], sourcesContent: [readFileSync(src, "utf8")], mappings: "" };
    writeFileSync(file + ".map", JSON.stringify(map));
    const recorded = [], observed = new Map([[file, { path: file, sha256: "sha256:" + createHash("sha256").update(readFileSync(file)).digest("hex") }]]);
    const result = captureFeedbackOptimizer(cache, path => recorded.push(path), observed);
    assert.equal(result.metadata.browserHash, "browser"); assert.equal(result.metadata.lockfileHash, "lock");
    assert.equal(result.dependencies[0].needsInterop, true); assert.equal(result.inputs.length, 3); assert(recorded.includes(src));
    writeFileSync(file, "export default 2;");
    assert.throws(() => captureFeedbackOptimizer(cache, () => {}, observed), /dependency changed/);
    const signals = join(root, "node_modules/@solidjs/signals"); mkdirSync(join(signals, "dist"), { recursive: true });
    writeFileSync(join(signals, "package.json"), '{"name":"@solidjs/signals"}');
    const reader = join(signals, "dist/dev-shared.js"); writeFileSync(reader, "substituted reader");
    map.sources = [reader]; writeFileSync(file + ".map", JSON.stringify(map));
    assert.throws(() => captureFeedbackOptimizer(cache, () => {}), /runtime was inlined/);
    const plugin = feedbackReadPlugin({ snapshot: {}, runtimeInputs: new Map(), coverage: {} });
    assert.throws(() => plugin.transform("substituted reader", reader), /reviewed runtime profile/);
    writeFileSync(join(signals, "package.json"), '{"name":"@solidjs/signals","version":"2.0.0-rc.13"}');
    assert.throws(() => plugin.transform("optimized reader", reader), /previous transform changed/);
    assert.throws(() => plugin.transform("substituted reader", reader), /reviewed development profile/);
    const guarded = { optimizeDeps: {} }, sources = new Map();
    const existing = { name: "application-optimizer" }; guarded.optimizeDeps.rolldownOptions = { plugins: existing };
    configureFeedbackOptimizer(guarded, { record() {}, sources });
    assert.equal(guarded.optimizeDeps.rolldownOptions.plugins[1], existing);
    assert.throws(() => guarded.optimizeDeps.rolldownOptions.plugins[0].transform("substituted reader", reader), /runtime reached dependency optimization/);
    let load;
    guarded.optimizeDeps.esbuildOptions.plugins[0].setup({ onLoad(_filter, callback) { load = callback; } });
    assert.throws(() => load({ path: reader }), /runtime reached dependency optimization/);
    rmSync(file + ".map"); assert.throws(() => captureFeedbackOptimizer(cache, () => {}), /no source inventory/);
    writeFileSync(file, 'export {value} from "./chunk.js";');
    const sourcesForFacades = new Map([["client", { environment: "client", id: src, sha256: "source" }],
      ["ssr", { environment: "ssr", id: src, sha256: "source" }]]);
    assert.doesNotThrow(() => captureFeedbackOptimizer(cache, () => {}, new Map(), sourcesForFacades));
    const ssr = join(cache, "deps_ssr"); mkdirSync(ssr);
    writeFileSync(join(ssr, "cjs.js"), 'export {value} from "./chunk.js";');
    writeFileSync(join(ssr, "_metadata.json"), JSON.stringify({ ...metadata, browserHash: "ssr-browser" }));
    const observer = createFeedbackOptimizerObserver(cache, () => {}, sourcesForFacades);
    observer.configure({ cacheDir: cache, environments: { client: {}, ssr: {} } });
    observer.observe(readFileSync(file, "utf8"), file + "?v=first");
    assert.equal(observer.finish().generations.find(row => row.environment === "ssr").metadata.browserHash, "ssr-browser");
    writeFileSync(file, 'export {other} from "./chunk.js";');
    assert.throws(() => observer.observe(readFileSync(file, "utf8"), file + "?v=first"), /changed within its generation/);
    writeFileSync(join(deps, "_metadata.json"), JSON.stringify({ ...metadata, browserHash: "second" }));
    observer.observe(readFileSync(file, "utf8"), file + "?v=second");
    assert.equal(observer.finish().generations.filter(row => row.environment === "client").length, 2);
    assert.throws(() => observer.observe("substituted bytes", file + "?v=second"), /previous transform changed/);
  } finally { rmSync(root, { recursive: true, force: true }); }
});

test("retired and processing optimizer directories never invent environments or require inventories", async () => {
  const root = realpathSync(mkdtempSync(join(tmpdir(), "solid-feedback-retired-")));
  try {
    const cache = join(root, "cache"); mkdirSync(cache);
    const src = join(root, "input.js"); writeFileSync(src, "export {};");
    const metadata = { hash: "hash", lockfileHash: "lock", configHash: "config", browserHash: "current",
      optimized: { cjs: { file: "facade.js", src } }, chunks: {} };
    for (const directory of ["deps", "deps_ssr", "deps_temp_retired", "deps_ssr_temp_retired", "deps_unknown"]) {
      mkdirSync(join(cache, directory));
      writeFileSync(join(cache, directory, "_metadata.json"), JSON.stringify(metadata));
      writeFileSync(join(cache, directory, "facade.js"), "export {};");
    }
    const sources = new Map(["client", "ssr"].map(environment => [environment, { environment, id: "actual-input" }]));
    let deleting = false, deleted = false;
    const retired = ["deps_temp_retired", "deps_ssr_temp_retired"];
    const observer = createFeedbackOptimizerObserver(cache, () => {
      if (deleting && !deleted) {
        for (const directory of retired) rmSync(join(cache, directory), { recursive: true });
        deleted = true;
      }
    }, sources);
    observer.configure({ cacheDir: cache, environments: { client: {}, ssr: {} } });
    assert.deepEqual(observer.finish().generations.map(row => row.environment), ["client", "ssr"]);
    // Deterministically delete retired outputs during a committed generation's
    // finalization, as Vite's asynchronous removal can do independently.
    deleting = true;
    assert.deepEqual(observer.finish().generations.map(row => row.environment), ["client", "ssr"]);
    assert(deleted);
    assert.throws(() => observer.observe("export {};", join(cache, "deps_unknown/facade.js")), /configured committed environment/);
  } finally { rmSync(root, { recursive: true, force: true }); }
});

test("configured cache aliases preserve Vite metadata's lexical relative source paths", () => {
  const root = realpathSync(mkdtempSync(join(tmpdir(), "solid-feedback-cache-alias-")));
  try {
    const actual = join(root, "deep/cache"), alias = join(root, "alias");
    mkdirSync(join(actual, "deps"), { recursive: true }); symlinkSync(actual, alias);
    const src = join(root, "input.js"); writeFileSync(src, "export {};");
    const file = join(alias, "deps/facade.js"); writeFileSync(file, "export {};");
    writeFileSync(join(alias, "deps/_metadata.json"), JSON.stringify({ hash: "hash", lockfileHash: "lock", configHash: "config",
      browserHash: "browser", optimized: { cjs: { file: "facade.js", src: "../../input.js" } }, chunks: {} }));
    const recorded = [], observer = createFeedbackOptimizerObserver(alias, path => recorded.push(path),
      new Map([["source", { environment: "client", id: src }]]));
    observer.configure({ cacheDir: alias, environments: { client: {} } });
    observer.observe("export {};", file);
    assert.equal(observer.finish().generations[0].dependencies[0].src, src);
    assert(recorded.includes(src));
  } finally { rmSync(root, { recursive: true, force: true }); }
});

test("Vite prebundles an included CommonJS carrier while its excluded Solid import stays external", async () => {
  const root = realpathSync(mkdtempSync(join(tmpdir(), "solid-feedback-interop-")));
  let server;
  try {
    const pkg = (name, code) => {
      const directory = join(root, "node_modules", name); mkdirSync(directory, { recursive: true });
      writeFileSync(join(directory, "package.json"), JSON.stringify({ name, main: "index.js" }));
      writeFileSync(join(directory, "index.js"), code); return join(directory, "index.js");
    };
    writeFileSync(join(root, "package.json"), '{"type":"module"}');
    const native = pkg("@solidjs/signals", "export const identity = 'one-runtime';");
    pkg("cjs-carrier", "const signals = require('@solidjs/signals'); module.exports = () => signals.identity;");
    writeFileSync(join(root, "entry.js"), "import carrier from 'cjs-carrier'; globalThis.carrier = carrier;");
    writeFileSync(join(root, "index.html"), '<script type="module" src="/entry.js"></script>');
    const cache = join(root, "cache");
    const sources = new Map(), recorded = [];
    const instances = new Map();
    server = await createServer({ root, configFile: false, cacheDir: cache, logLevel: "silent",
      server: { host: "127.0.0.1", port: 0 }, optimizeDeps: { include: ["cjs-carrier"], holdUntilCrawlEnd: false },
      plugins: [{ name: "collector-optimizer", enforce: "pre",
        resolveId(id, importer, options) { return resolveFeedbackRuntime(id, importer, options, this.resolve.bind(this), instances); },
        configResolved(config) { configureFeedbackOptimizer(config, { record: path => recorded.push(path), sources }); } }] });
    await server.listen(); await server.transformRequest("/entry.js");
    for (let index = 0; index < 100 && !existsSync(join(cache, "deps/_metadata.json")); index++) await new Promise(resolve => setTimeout(resolve, 20));
    assert(existsSync(join(cache, "deps/_metadata.json")), "Vite did not commit dependency metadata");
    const result = captureFeedbackOptimizer(cache, () => {}, new Map(), sources);
    assert.deepEqual(result.dependencies.map(row => row.id), ["cjs-carrier"]);
    const output = readFileSync(result.dependencies[0].file, "utf8");
    assert.match(output, /@solidjs\/signals/); assert(!output.includes("one-runtime"));
    assert(result.sources.some(row => row.path === join(root, "node_modules/cjs-carrier/index.js")));
    assert(!recorded.includes(native));
    const client = server.environments.client;
    const first = await client.pluginContainer.resolveId("@solidjs/signals", join(root, "entry.js"));
    client.depsOptimizer.metadata.browserHash = "changed-generation";
    const second = await client.pluginContainer.resolveId("@solidjs/signals", join(root, "entry.js"));
    assert.equal(first.id, native); assert.equal(second.id, native);
    const transformed = await server.transformRequest("/node_modules/@solidjs/signals/index.js");
    assert.equal(transformed.code.trim(), readFileSync(native, "utf8").trim());
  } finally { if (server) await server.close(); rmSync(root, { recursive: true, force: true }); }
}, 10000);

test("runtime version queries are stable, other queries survive, and separate installed runtime roots refuse", async () => {
  const root = realpathSync(mkdtempSync(join(tmpdir(), "solid-feedback-runtime-id-")));
  try {
    const paths = ["a", "b"].map(name => {
      const directory = join(root, name); mkdirSync(directory);
      writeFileSync(join(directory, "package.json"), '{"name":"@solidjs/signals"}');
      const path = join(directory, "runtime.js"); writeFileSync(path, "export const value=1;"); return path;
    });
    const instances = new Map(), record = [];
    const resolve = async id => ({ id, meta: { preserved: true } });
    const first = await resolveFeedbackRuntime(paths[0] + "?v=one&raw", undefined, {}, resolve, instances, path => record.push(path));
    assert.equal(first.id, paths[0] + "?raw"); assert.equal(first.meta.preserved, true);
    const second = await resolveFeedbackRuntime(paths[0] + "?v=two", undefined, {}, resolve, instances);
    assert.equal(second.id, paths[0]); assert(record.includes(join(root, "a/package.json")));
    await assert.rejects(resolveFeedbackRuntime(paths[1], undefined, {}, resolve, instances), /Multiple installed Solid runtime instances/);
    assert.equal(await resolveFeedbackRuntime("unknown", undefined, {}, async () => null, instances), null);
  } finally { rmSync(root, { recursive: true, force: true }); }
});

test("the transform boundary compares Vite's already-decoded request without decoding it again", () => {
  // Selection normalizes the raw HTML URL once, as Vite's HTTP middleware does.
  const entries = ["/src/main.ts?label=100%25", "/src/main.ts?code=%2561"]
    .map((url, index) => ({ url, request: normalizeFeedbackEntryRequest(url), id: `/abs/main-${index}` }));
  assert.equal(entries[0].request, "/src/main.ts?label=100%");
  assert.equal(entries[1].request, "/src/main.ts?code=%61");
  // Vite then passes those decoded forms to transformRequest: they must select
  // without throwing on a literal `%`, and `%61` must not become `a`.
  assert.equal(selectedFeedbackEntry(entries, "/src/main.ts?label=100%"), entries[0]);
  assert.equal(selectedFeedbackEntry(entries, "/src/main.ts?code=%61"), entries[1]);
  assert.equal(selectedFeedbackEntry(entries, "/src/main.ts?code=a"), undefined);
  // An unselected dependency with a literal-percent query is simply not selected.
  assert.equal(selectedFeedbackEntry(entries, "/src/dep.ts?x=5%"), undefined);
});
