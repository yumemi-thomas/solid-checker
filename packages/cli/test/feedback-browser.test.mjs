import assert from "node:assert/strict";
import { test } from "vitest";
import { createReadCollector } from "../scripts/feedback-read-runtime.mjs";
import { selectAssertionFeedback } from "../scripts/feedback-assertion-selector.mjs";
import { closeFeedbackResources, createFeedbackDiagnostics, executeFeedbackScenario, feedbackContextOptions, feedbackReadPlugin, loadFeedbackConfiguration, loadSuppliedResponses,
  mergeFeedbackServerConfiguration, recordFeedbackGeneratorInputs, routeFeedbackRequest, selectFeedbackModels, validateFeedbackRuntimeOptions, validateFeedbackScenario } from "../scripts/feedback-browser.mjs";
import { existsSync, mkdirSync, mkdtempSync, realpathSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { instrumentFeedbackReads } from "../scripts/feedback-native-hook.mjs";

const row = (actual, expected = "2") => ({ id: "value", selector: "#value", actual, expected });
test("capture transforms precede the application's pre compiler while preserving resolution settings", () => {
  const capture = { name: "capture", enforce: "pre" }, compiler = { name: "compiler", enforce: "pre" };
  const application = { plugins: [compiler], publicDir: "static", resolve: { alias: { "solid-js/web": "@solidjs/web" } } };
  const collector = { plugins: [capture], cacheDir: "/temporary" };
  const result = mergeFeedbackServerConfiguration({ mergeConfig: (app, capture) => ({ ...app, ...capture }) }, application, collector);
  assert.deepEqual(result.plugins, [capture, compiler]); assert.equal(result.publicDir, "static");
  assert.equal(result.resolve, application.resolve); assert.equal(result.cacheDir, "/temporary");
  assert.deepEqual(application.plugins, [compiler]);
});
test("application configuration preserves aliases, assets and compiler plugins without a second compiler", async () => {
  const path = fileURLToPath(import.meta.url), plugin = { name: "application-compiler" };
  const config = { plugins: [plugin], publicDir: "static", resolve: { alias: { "solid-js/web": "@solidjs/web" } } };
  const pins = [], tools = { async loadConfigFromFile(environment, file, root) {
    assert.equal(environment.command, "serve"); assert.equal(file, undefined); assert.equal(root, "/application");
    return { path, dependencies: [path], config };
  }, solid() { assert.fail("application owns its compiler plugin"); } };
  const result = await loadFeedbackConfiguration(tools, "/application", path => pins.push(path));
  assert.equal(result.config, config); assert.deepEqual(result.plugins, []); assert.deepEqual(pins, [path]);
  assert.equal(result.configuration.kind, "application-vite-config");
});
test("a project without Vite configuration keeps the collector's compiler default", async () => {
  const plugin = { name: "default-compiler" };
  const result = await loadFeedbackConfiguration({ loadConfigFromFile: async () => null, solid: async () => plugin }, "/application", () => assert.fail());
  assert.deepEqual(result.plugins, [plugin]); assert.equal(result.configuration.kind, "collector-default");
});
test("an explicit root owns automatic config lookup while the source leaf stays separate", async () => {
  const path = fileURLToPath(import.meta.url);
  const result = await loadFeedbackConfiguration({ loadConfigFromFile: async (_env, file, root) => {
    assert.equal(file, undefined); assert.equal(root, "/application/runtime");
    return { path, dependencies: [], config: {} };
  } }, "/application/source-leaf", () => {}, { root: "/application/runtime" });
  assert.equal(result.root, "/application/runtime");
});
test("failed readiness retains preceding assertions and stops dependent interactions", async () => {
  const calls = [], page = { locator(selector) { calls.push(selector); return {
    textContent: async () => "ready", waitFor: async () => { throw new Error("missing hydrated element"); },
    click: async () => assert.fail("dependent interaction must not execute")
  }; }, evaluate: async () => ({ events: 0 }) };
  const result = await executeFeedbackScenario(page, { steps: [
    { action: "assert-text", id: "ready", selector: "h1", text: "ready" },
    { action: "wait-for-selector", selector: "#hydrated" }, { action: "click", selector: "button" }
  ] });
  assert.equal(result.assertions[0].passed, true); assert.equal(result.failure.step, 1);
  assert.equal(result.failure.selector, "#hydrated"); assert.equal(result.failure.authority, false);
  assert.deepEqual(calls, ["h1", "#hydrated"]);
});
test("measured comparison satisfying a failed supplied assertion receives bounded debugging guidance", () => {
  const result = selectAssertionFeedback([row("1")], [row("2")]);
  assert.equal(result.notes.length, 1); assert.equal(result.notes[0].severity, "info");
  assert.equal(result.notes[0].repairSafety, "unproved"); assert.equal(result.notes[0].authority, false);
});
test("a passing snapshot assertion stays quiet when comparison also passes", () => {
  const result = selectAssertionFeedback([row("9", "9")], [row("9", "9")]);
  assert.deepEqual(result.notes, []); assert.equal(result.unchanged.length, 1);
});
test("breaking a passing identity assertion stays open", () => {
  const result = selectAssertionFeedback([row("1", "1")], [row("2", "1")]);
  assert.deepEqual(result.notes, []); assert.match(result.open[0].reason, /breaks/);
});
test("a changed result that still fails the assertion supplies no repair guidance", () => {
  const result = selectAssertionFeedback([row("1", "3")], [row("2", "3")]);
  assert.deepEqual(result.notes, []); assert.match(result.open[0].reason, /does not satisfy/);
});
for (const comparison of [[], [{ ...row("2"), id: "other" }], [{ ...row("2"), selector: "#other" }], [row("2", "3")]]) {
  test(`no matching supplied assertion stays open: ${JSON.stringify(comparison)}`, () => {
    const result = selectAssertionFeedback([row("1")], comparison);
    assert.deepEqual(result.notes, []); assert.equal(result.open.length, 1);
  });
}
test("collector preserves returned object identity and only records normal unowned reads", () => {
  const collector = createReadCollector(), node = {}, value = {};
  const ticket = collector.begin(node, () => null, () => null);
  assert.equal(collector.finish(value, ticket), value); assert.equal(collector.events.length, 1);
  assert.equal(collector.begin(node, () => ({}), () => null), null);
  assert.equal(collector.begin(node, () => null, () => ({})), null);
});
test("native untrack suppresses observations and restores its previous scope", () => {
  const collector = createReadCollector(), node = {};
  const previous = collector.enterIntent();
  assert.equal(collector.begin(node, () => null, () => null), null);
  collector.leaveIntent(previous);
  assert(collector.begin(node, () => null, () => null));
});
test("failed reads have no normal-return observation", () => {
  const collector = createReadCollector(); collector.begin({}, () => null, () => null);
  assert.equal(collector.events.length, 0);
});
test("event and byte budgets explicitly report incomplete coverage", () => {
  const collector = createReadCollector({ maxEvents: 1 });
  for (let i = 0; i < 3; i++) collector.finish(i, collector.begin({}, () => null, () => null));
  assert.equal(collector.events.length, 1); assert.equal(collector.stats.dropped, 2); assert.equal(collector.stats.complete, false);
  const small = createReadCollector({ maxBytes: 1 }); small.finish(1, small.begin({}, () => null, () => null));
  assert.equal(small.events.length, 0); assert.equal(small.stats.dropped, 1);
});
test("native node identity stays weak and stable across reads", () => {
  const collector = createReadCollector(), node = {};
  assert.equal(collector.begin(node, () => null, () => null).nodeId, collector.begin(node, () => null, () => null).nodeId);
});
test("native bytes outside the reviewed profile refuse instrumentation", () => {
  assert.throws(() => instrumentFeedbackReads("export function read(){}", "/native.js", "/runtime.js"), /reviewed development profile/);
});
test("scenario accepts explicit authored interactions and assertions", () => {
  const scenario = { schemaVersion: 1, steps: [{ action: "click", selector: "button" },
    { action: "assert-text", id: "value", selector: "#value", text: "2" }] };
  assert.equal(validateFeedbackScenario(scenario), scenario);
});
test("automatic collection accepts interactions without assertions", () => {
  assert.doesNotThrow(() => validateFeedbackScenario({ schemaVersion: 1, steps: [{ action: "click", selector: "button" }] }));
});
for (const steps of [[], [{ action: "evaluate", selector: "body" }],
  [1, 2].map(() => ({ action: "assert-text", id: "same", selector: "#value", text: "2" }))]) {
  test(`scenario refuses ambiguous or unsupported actions: ${JSON.stringify(steps)}`, () => {
    assert.throws(() => validateFeedbackScenario({ schemaVersion: 1, steps }));
  });
}

const click = [{ action: "click", selector: "button" }];
test("supplied responses bind one exact URL to a digest-pinned body", () => {
  const directory = mkdtempSync(join(tmpdir(), "solid-feedback-responses-"));
  try {
    writeFileSync(join(directory, "articles.json"), "[]");
    const scenario = validateFeedbackScenario({ schemaVersion: 1, steps: click,
      responses: [{ url: "https://api.example.test/articles?user=a", body: "articles.json" }] });
    const responses = loadSuppliedResponses(scenario, join(directory, "scenario.json"));
    const row = responses.get("https://api.example.test/articles?user=a");
    assert.equal(row.status, 200); assert.equal(row.contentType, "application/json"); assert.equal(row.body.toString(), "[]");
    assert.equal(row.pin.sha256, "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945");
    assert.equal(responses.size, 1);
  } finally { rmSync(directory, { recursive: true, force: true }); }
});
for (const responses of [{}, [{ url: "relative", body: "a.json" }], [{ url: "https://Example.test/a", body: "a.json" }],
  [{ url: "ftp://example.test/a", body: "a.json" }], [{ url: "https://example.test/a" }],
  [{ url: "https://example.test/a", body: "a.json", status: 99 }],
  [1, 2].map(() => ({ url: "https://example.test/a", body: "a.json" }))]) {
  test(`scenario refuses inexact or incomplete supplied responses: ${JSON.stringify(responses)}`, () => {
    assert.throws(() => validateFeedbackScenario({ schemaVersion: 1, steps: click, responses }));
  });
}

test("records from every loaded document survive a full page load, and a silent document is counted lost", async () => {
  const { mergeDocumentStates } = await import("../scripts/feedback-browser.mjs");
  const state = (reads, nodeId, entered) => ({ events: [{ kind: "untracked-read", nodeId }], diagnostics: [{ code: reads }],
    stats: { reads, observerQueries: 1, retained: 1, dropped: 0, bytes: 10, complete: false },
    scopes: { rows: [{ kind: "operation", path: "/a.ts", span: { start: 1, end: 2 }, entered, returned: entered, threw: 0 }], dropped: 0 } });
  const merged = mergeDocumentStates([{ phase: "start", id: "a" }, { phase: "end", id: "a", state: state(5, 1, 2) },
    { phase: "start", id: "b" }], { id: "b", state: state(3, 1, 1) });
  assert.deepEqual(merged.documents, { loaded: 2, reported: 2, lost: 0 });
  assert.deepEqual(merged.events.map(event => [event.document, event.nodeId]), [[0, 1], [1, 1]]);
  assert.equal(merged.stats.reads, 8); assert.equal(merged.stats.complete, false); assert.equal(merged.diagnostics.length, 2);
  assert.deepEqual(merged.scopes.rows.map(row => [row.entered, row.returned, row.threw]), [[3, 3, 0]]);
  const silent = mergeDocumentStates([{ phase: "start", id: "a" }, { phase: "start", id: "b" }], { id: "b", state: state(3, 1, 1) });
  assert.deepEqual(silent.documents, { loaded: 2, reported: 1, lost: 1 }); assert.equal(silent.events[0].document, 1);
  assert.deepEqual(mergeDocumentStates([], null).documents, { loaded: 0, reported: 0, lost: 0 });
});

test("root and config are existing canonical inputs inside the app workspace, independent of its source leaf", async () => {
  const directory = realpathSync(mkdtempSync(join(tmpdir(), "solid-feedback-options-")));
  try {
    mkdirSync(join(directory, "apps/web"), { recursive: true });
    writeFileSync(join(directory, "package.json"), '{"workspaces":["apps/*"]}');
    writeFileSync(join(directory, "apps/web/package.json"), '{}');
    writeFileSync(join(directory, "apps/web/tsconfig.json"), '{}');
    const config = join(directory, "vite.config.mjs"); writeFileSync(config, 'export default {};');
    const project = join(directory, "apps/web/tsconfig.json"), root = join(directory, "apps/web");
    const options = validateFeedbackRuntimeOptions(project, { root, config });
    assert.equal(options.appTree, directory); assert.equal(options.root, root); assert.equal(options.config, config);
    const pins = [];
    const loaded = await loadFeedbackConfiguration({ loadConfigFromFile: async (_env, file, projectRoot) => {
      assert.equal(file, config); assert.equal(projectRoot, join(directory, "apps/web"));
      return { path: config, dependencies: [config], config: { root: "ignored" } };
    } }, join(directory, "apps/web"), path => pins.push(path), options);
    assert.equal(loaded.root, root); assert.deepEqual(pins, [config]);
    assert.throws(() => validateFeedbackRuntimeOptions(project, { root: config }), /directory/);
    assert.throws(() => validateFeedbackRuntimeOptions(project, { config: root }), /file/);
    assert.throws(() => validateFeedbackRuntimeOptions(project, { root: join(directory, "missing") }), /ENOENT/);
    assert.throws(() => validateFeedbackRuntimeOptions(project, { config: fileURLToPath(import.meta.url) }), /outside the app tree/);
    symlinkSync(tmpdir(), join(directory, "escape"));
    assert.throws(() => validateFeedbackRuntimeOptions(project, { root: join(directory, "escape") }), /outside the app tree/);
    await assert.rejects(loadFeedbackConfiguration({ loadConfigFromFile: async () => null }, root, () => {}, options), /could not be loaded/);
  } finally { rmSync(directory, { recursive: true, force: true }); }
});

const inputs = [
  { action: "fill", selector: "input[name=query]", text: "" },
  { action: "type", selector: "input[name=query]", text: "solid" },
  { action: "press", selector: "input[name=query]", key: "Enter" },
  { action: "select", selector: "select", value: "all" },
  { action: "check", selector: "input[type=checkbox]" },
  { action: "uncheck", selector: "input[type=checkbox]" },
  { action: "submit", selector: "form" }
];
for (const step of inputs) test(`input action ${step.action} executes and retains its CSS checkpoint`, async () => {
  const scenario = validateFeedbackScenario({ schemaVersion: 1, steps: [step] }), calls = [];
  class Form { requestSubmit() { calls.push(["submit"]); } }
  const previousForm = globalThis.HTMLFormElement, previousDocument = globalThis.document;
  globalThis.HTMLFormElement = Form;
  globalThis.document = { querySelector(selector) { assert.equal(selector, step.selector); return { textContent: "checkpoint" }; } };
  try {
    const locator = Object.fromEntries(["fill", "pressSequentially", "press", "selectOption", "check", "uncheck"].map(name =>
      [name, async (...args) => calls.push([name, ...args])]));
    locator.evaluate = async fn => fn(new Form());
    const page = { locator: selector => { assert.equal(selector, step.selector); return locator; }, evaluate: async (fn, arg) => fn(arg) };
    const run = await executeFeedbackScenario(page, scenario);
    const method = { type: "pressSequentially", select: "selectOption" }[step.action] ?? step.action;
    assert.deepEqual(calls, [[method, ...("text" in step ? [step.text] : "key" in step ? [step.key] : "value" in step ? [step.value] : [])]]);
    assert.equal(run.failure, null); assert.equal(run.checkpoints[0].text, "checkpoint");
  } finally { globalThis.HTMLFormElement = previousForm; globalThis.document = previousDocument; }
});
for (const step of [
  { action: "fill", selector: "input" }, { action: "type", selector: "input", text: 1 },
  { action: "press", selector: "input", key: "" }, { action: "select", selector: "select", value: 1 },
  ...["check", "uncheck", "submit"].map(action => ({ action, selector: "" })),
  { action: "fill", selector: "text=search", text: "a" }
]) test(`input action refuses missing or invalid data: ${JSON.stringify(step)}`, () => {
  assert.throws(() => validateFeedbackScenario({ schemaVersion: 1, steps: [step] }));
});
test("submit refuses a non-form and prevents following actions", async () => {
  const previous = globalThis.HTMLFormElement; globalThis.HTMLFormElement = class {};
  try {
    const run = await executeFeedbackScenario({ locator: () => ({ evaluate: async fn => fn({}) }) }, { steps: [inputs.at(-1), inputs[0]] });
    assert.match(run.failure.message, /must identify a form/); assert.equal(run.failure.step, 0);
  } finally { globalThis.HTMLFormElement = previous; }
});
test("viewport dimensions and optional device settings pass to the browser context", () => {
  const viewport = { width: 390, height: 844, deviceScaleFactor: 2, isMobile: true };
  const scenario = validateFeedbackScenario({ schemaVersion: 1, steps: click, viewport });
  assert.deepEqual(feedbackContextOptions(scenario), { viewport: { width: 390, height: 844 }, deviceScaleFactor: 2, isMobile: true });
  assert.deepEqual(feedbackContextOptions({ steps: click }), {});
});
for (const viewport of [null, [], {}, { width: 0, height: 100 }, { width: 100.5, height: 100 },
  { width: 100, height: 16385 }, { width: 100, height: 100, deviceScaleFactor: 0 },
  { width: 100, height: 100, deviceScaleFactor: 9 }, { width: 100, height: 100, isMobile: "yes" },
  { width: 100, height: 100, unknown: true }]) test(`invalid viewport refuses: ${JSON.stringify(viewport)}`, () => {
  assert.throws(() => validateFeedbackScenario({ schemaVersion: 1, steps: click, viewport }));
});
test("same-origin responses precede proxy traffic and match the exact query, method and POST body", async () => {
  const directory = mkdtempSync(join(tmpdir(), "solid-feedback-offline-"));
  try {
    writeFileSync(join(directory, "body.json"), '{"ok":true}');
    const scenario = validateFeedbackScenario({ schemaVersion: 1, steps: click, responses: [
      { url: "/api/user?mode=offline", body: "body.json" },
      { url: "/api/user?mode=offline", method: "POST", requestBody: '{"name":"a"}', body: "body.json", status: 201 },
      { url: "https://other.test/a", body: "body.json" }
    ] });
    const responses = loadSuppliedResponses(scenario, join(directory, "scenario.json")), blocked = [];
    const route = async (url, method = "GET", body = null) => {
      let result;
      await routeFeedbackRequest({ request: () => ({ url: () => url, method: () => method, postData: () => body }),
        fulfill: async options => { result = options; }, continue: async () => { result = "proxy"; }, abort: async () => { result = "blocked"; }
      }, "http://127.0.0.1:1234", responses, blocked);
      return result;
    };
    assert.equal((await route("http://127.0.0.1:1234/api/user?mode=offline")).status, 200);
    assert.equal((await route("http://127.0.0.1:1234/api/user?mode=offline", "POST", '{"name":"a"}')).status, 201);
    assert.equal(await route("http://127.0.0.1:1234/api/user?mode=other"), "proxy");
    assert.equal(await route("http://127.0.0.1:1234/api/user?mode=offline", "POST", '{"name":"b"}'), "proxy");
    assert.equal(await route("http://127.0.0.1:1234/api/user?mode=offline", "HEAD"), "proxy");
    assert.equal((await route("https://other.test/a")).status, 200);
    assert.equal(await route("https://other.test/a", "POST", ""), "blocked");
    assert.deepEqual([...responses.values()].map(row => row.served), [1, 1, 1]);
    assert.match([...responses.values()][1].requestBodySha256, /^sha256:/);
  } finally { rmSync(directory, { recursive: true, force: true }); }
});
for (const response of [
  { url: "//other.test/a" }, { url: "/a/../b" }, { url: "/api#fragment" }, { url: "/api", method: "PUT" },
  { url: "/api", method: "POST" }, { url: "/api", requestBody: "" }, { url: "/api", method: "POST", requestBody: {} }
]) test(`inexact same-origin response refuses: ${JSON.stringify(response)}`, () => {
  assert.throws(() => validateFeedbackScenario({ schemaVersion: 1, steps: click, responses: [{ body: "a.json", ...response }] }));
});
test("Vite diagnostics and failing module bodies are bounded and retained without inferring a cause", async () => {
  const diagnostics = createFeedbackDiagnostics(), origin = "http://127.0.0.1:1234";
  for (let i = 0; i < 65; i++) diagnostics.logger.error("transform failed " + "x".repeat(20000));
  diagnostics.response({ url: () => origin + "/src/App.tsx", status: () => 500, headers: () => ({ "content-type": "text/plain" }),
    request: () => ({ resourceType: () => "script" }), body: async () => Buffer.from("compiler error " + "x".repeat(20000)) }, origin);
  diagnostics.response({ url: () => "https://foreign.test/a" }, origin);
  const result = await diagnostics.finish();
  assert.equal(result.vite.length, 64); assert.equal(result.viteDropped, 1); assert.equal(result.vite[0].truncated, true);
  assert.equal(result.failedModules.length, 1); assert.equal(result.failedModules[0].status, 500);
  assert.match(result.failedModules[0].body.text, /^compiler error/); assert.equal(result.failedModules[0].body.truncated, true);
});
test("cleanup closes every resource and removes this cache even when context close fails", async () => {
  const cache = mkdtempSync(join(tmpdir(), "solid-feedback-cleanup-")), calls = [];
  await assert.rejects(closeFeedbackResources({ cache,
    context: { close: async () => { calls.push("context"); throw new Error("close failed"); } },
    server: { close: async () => calls.push("server") }, browser: { close: async () => calls.push("browser") }
  }), /cleanup failed/);
  assert.deepEqual(calls, ["context", "server", "browser"]); assert.equal(existsSync(cache), false);
});
test("shared leaf models are deduplicated, differing models remain open, and differing source bytes refuse", () => {
  const model = { path: "/app/source.ts", sourceSha256: "same", functions: [{ span: { start: 0, end: 10 } }] };
  assert.equal(selectFeedbackModels([model, structuredClone(model)]).models.size, 1);
  const differing = { ...model, functions: [] };
  const result = selectFeedbackModels([model, differing, model]);
  assert.equal(result.models.size, 0); assert.deepEqual([...result.conflictingLeafModels], [model.path]);
  assert.throws(() => selectFeedbackModels([model, differing, { ...model, sourceSha256: "changed" }]), /source changed/);
});
test("config-generated client modules collect diagnostics even when middleware bypasses HTML transforms", () => {
  const coverage = {}, generatedModules = new Map([["virtual:app-entry", {}]]);
  const plugin = feedbackReadPlugin({ coverage, generatedModules }), source = 'import App from "/src/App.tsx";';
  const result = plugin.transform(source, "virtual:app-entry", {});
  assert.equal(result.code, 'import "virtual:solid-checker-feedback-entry";\n' + source);
  assert.deepEqual(coverage.generatedBootstrap, ["virtual:app-entry"]);
  assert.equal(plugin.transform(source, "virtual:app-entry", { ssr: true }), null);
  assert.equal(plugin.transform.call({ environment: { config: { consumer: "server" } } }, source, "virtual:app-entry", {}), null);
  assert.equal(plugin.transform(source, "/src/App.tsx", {}), null);
});
test("generated-entry inputs pin externalized config packages and their import/require artifacts and local helpers", () => {
  const root = realpathSync(mkdtempSync(join(tmpdir(), "solid-feedback-generator-pins-")));
  try {
    const pkg = join(root, "node_modules/shell-plugin"); mkdirSync(pkg, { recursive: true });
    const config = join(root, "vite.config.mjs"); writeFileSync(config, 'import shell from "shell-plugin"; export default {plugins:[shell()]};');
    writeFileSync(join(pkg, "package.json"), JSON.stringify({ name: "shell-plugin", type: "module",
      exports: { ".": { import: "./generator.mjs", require: "./generator.cjs" } } }));
    writeFileSync(join(pkg, "generator.mjs"), 'import "./helper.mjs"; export default () => ({});');
    writeFileSync(join(pkg, "generator.cjs"), 'module.exports = () => ({});');
    writeFileSync(join(pkg, "helper.mjs"), 'export const html = "app";');
    const pins = new Set(); recordFeedbackGeneratorInputs([config], path => pins.add(path));
    for (const file of ["package.json", "generator.mjs", "generator.cjs", "helper.mjs"]) assert(pins.has(join(pkg, file)), file);
    assert(pins.has(config));
  } finally { rmSync(root, { recursive: true, force: true }); }
});
