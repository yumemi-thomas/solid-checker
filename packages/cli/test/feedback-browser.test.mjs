import assert from "node:assert/strict";
import { test } from "vitest";
import { createReadCollector } from "../scripts/feedback-read-runtime.mjs";
import { selectAssertionFeedback } from "../scripts/feedback-assertion-selector.mjs";
import { closeFeedbackResources, closeFeedbackResourcesAfter, validateFeedbackRequestRouting, createFeedbackDiagnostics, executeFeedbackScenario, feedbackContextOptions, feedbackPhysicalEntry, loadFeedbackConfiguration, loadSuppliedResponses,
  mergeFeedbackServerConfiguration, routeFeedbackRequest, selectFeedbackModels, validateFeedbackRoot, validateFeedbackRuntimeOptions, validateFeedbackScenario } from "../scripts/feedback-browser.mjs";
import { existsSync, mkdirSync, mkdtempSync, realpathSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { instrumentFeedbackReads } from "../scripts/feedback-native-hook.mjs";
import { createHash } from "node:crypto";
import { EventEmitter } from "node:events";

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
    const run = await executeFeedbackScenario({ evaluate: async () => {}, locator: () => ({ evaluate: async fn => fn({}) }) }, { steps: [inputs.at(-1), inputs[0]] });
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
      await routeFeedbackRequest({ request: () => ({ url: () => url, method: () => method, postDataBuffer: () => body === null ? null : Buffer.from(body), resourceType: () => "fetch" }),
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

test("same-origin supplied scripts/documents/stylesheets are refused and recorded; fetch/xhr remain data inputs", async () => {
  const origin = "http://127.0.0.1:1234", supplied = { url: "/src/entry.ts", method: "GET", body: Buffer.from("different code"), served: 0 };
  const responses = new Map([[supplied.url, supplied]]), refused = [];
  for (const resourceType of ["script", "document", "stylesheet", "fetch", "xhr"]) {
    let outcome;
    await routeFeedbackRequest({ request: () => ({ url: () => origin + supplied.url, method: () => "GET", resourceType: () => resourceType }),
      fulfill: async () => { outcome = "supplied"; }, abort: async () => { outcome = "refused"; }, continue: () => assert.fail()
    }, origin, responses, [], refused);
    assert.equal(outcome, ["fetch", "xhr"].includes(resourceType) ? "supplied" : "refused");
  }
  assert.equal(supplied.served, 2);
  assert.deepEqual(refused.map(row => row.resourceType), ["script", "document", "stylesheet"]);
  assert(refused.every(row => row.reason.includes("fetch/xhr")));
});

for (const [expected, actual, matches] of [["�", Buffer.from([0x80]), false], ["日本語", Buffer.from("日本語"), true],
  ["", Buffer.alloc(0), true], ["", null, false], ["a", Buffer.from("b"), false]]) {
  test(`POST matching uses complete UTF-8 bytes: ${JSON.stringify(expected)}/${actual?.toString("hex") ?? "unavailable"}`, async () => {
    const supplied = { url: "/api", method: "POST", requestBody: expected, served: 0 }, responses = new Map([["post", supplied]]);
    let fulfilled = false;
    await routeFeedbackRequest({ request: () => ({ url: () => "http://app.test/api", method: () => "POST", postDataBuffer: () => actual, resourceType: () => "xhr" }),
      fulfill: async () => { fulfilled = true; }, continue: async () => {}, abort: () => assert.fail()
    }, "http://app.test", responses, []);
    assert.equal(fulfilled, matches); assert.equal(supplied.served, matches ? 1 : 0);
    assert.equal(supplied.matchedRequestBodySha256, matches ? "sha256:" + createHash("sha256").update(actual).digest("hex") : undefined);
  });
}

test("effective roots refuse config-only escapes and hook changes; physical entries require in-tree files", () => {
  const root = realpathSync(mkdtempSync(join(tmpdir(), "solid-feedback-effective-root-")));
  try {
    mkdirSync(join(root, "changed"));
    assert.equal(validateFeedbackRoot(root, root, root), root);
    assert.throws(() => validateFeedbackRoot(tmpdir(), root), /outside the app tree/);
    assert.throws(() => validateFeedbackRoot(join(root, "changed"), root, root), /changed the reported root/);
    assert.throws(() => feedbackPhysicalEntry(root, root), /needs an application index.html/);
    symlinkSync(fileURLToPath(import.meta.url), join(root, "index.html"));
    assert.throws(() => feedbackPhysicalEntry(root, root), /inside the app tree/);
  } finally { rmSync(root, { recursive: true, force: true }); }
});

for (const selector of [" text=search", "div >> text=search", "div:has-text('search')", "input:visible", "div:nth-match(1)"]) {
  test(`Playwright selector syntax refuses before mutation: ${selector}`, async () => {
    assert.throws(() => validateFeedbackScenario({ schemaVersion: 1, steps: [{ action: "fill", selector, text: "x" }] }), /native CSS/);
    const result = await executeFeedbackScenario({ locator: () => assert.fail("must not mutate") },
      { steps: [{ action: "fill", selector, text: "x" }] });
    assert.match(result.failure.message, /native CSS/);
  });
}
test("selector syntax inside CSS strings and escapes is not taken for a Playwright engine", () => {
  for (const selector of ['button[class~="hover:text-white"]', '[data-label="a >> b"]', ".hover\\:text-white", "[title='x:visible']"])
    validateFeedbackScenario({ schemaVersion: 1, steps: [{ action: "click", selector }] });
});
test("native CSS parsing precedes an action and refuses malformed selectors", async () => {
  let parsed = false, filled = false;
  const page = { evaluate: async (_fn, selector) => {
    if (selector === "[") throw new Error("Invalid native CSS selector");
    parsed = true;
  }, locator: () => ({ fill: async () => { assert(parsed); filled = true; } }) };
  const failed = await executeFeedbackScenario(page, { steps: [{ action: "fill", selector: "[", text: "x" }] });
  assert.equal(filled, false); assert.match(failed.failure.message, /native CSS/);
  const valid = await executeFeedbackScenario(page, { steps: [{ action: "fill", selector: "input[data-name = 'q']", text: "x" }] });
  assert.equal(valid.failure, null); assert.equal(filled, true);
});

function diagnosticResponse(diagnostics, origin, path) {
  let middleware;
  diagnostics.install({ middlewares: { use(fn) { middleware = fn; } } });
  const response = Object.assign(new EventEmitter(), { statusCode: 500, writableFinished: false,
    writeHead() {}, write() { return true; }, end() { this.writableFinished = true; this.emit("finish"); } });
  middleware({ url: path }, response, () => {});
  response.writeHead(500);
  diagnostics.response({ url: () => origin + path, status: () => 500, headers: () => ({ "content-type": "text/plain" }),
    request: () => ({ resourceType: () => "script" }), body: () => assert.fail("unbounded body acquisition is forbidden") }, origin);
  return response;
}
test("failed module diagnostics capture original streaming bytes with a read cap and logger row limits", async () => {
  const diagnostics = createFeedbackDiagnostics(), origin = "http://127.0.0.1:1234";
  for (let i = 0; i < 65; i++) diagnostics.logger.error("transform failed " + "x".repeat(20000));
  const response = diagnosticResponse(diagnostics, origin, "/src/App.tsx");
  response.write("compiler error " + "x".repeat(1000000)); response.end("tail");
  const result = await diagnostics.finish(); diagnostics.dispose();
  assert.equal(result.vite.length, 64); assert.equal(result.viteDropped, 1); assert.equal(result.vite[0].truncated, true);
  const row = result.failedModules[0];
  assert.equal(row.status, 500); assert.match(row.body.text, /^compiler error/);
  assert.equal(Buffer.byteLength(row.body.text), 16384); assert.equal(row.body.bytes, 1000019);
  assert.equal(row.body.truncated, true); assert.equal(row.body.timedOut, false);
});
test("a failed body is attached only when its server capture is exactly associated", async () => {
  const origin = "http://app.test";
  // A failed fetch then a failed script at the same URL: the capture cannot be bound to the script.
  const ambiguous = createFeedbackDiagnostics();
  let middleware;
  ambiguous.install({ middlewares: { use(fn) { middleware = fn; } } });
  const serve = (status, text) => {
    const response = Object.assign(new EventEmitter(), { statusCode: status, writableFinished: false,
      writeHead() {}, write() { return true }, end() { this.writableFinished = true; this.emit("finish"); } });
    middleware({ url: "/src/lazy.js" }, response, () => {});
    response.writeHead(status); response.end(text);
  };
  serve(404, '{"error":"fetch"}'); serve(500, "compiler error");
  ambiguous.response({ url: () => origin + "/src/lazy.js", status: () => 500, headers: () => ({}),
    request: () => ({ resourceType: () => "script" }), body: () => assert.fail("unbounded body acquisition is forbidden") }, origin);
  const first = await ambiguous.finish(); ambiguous.dispose();
  assert.equal(first.failedModules[0].body, undefined);
  assert.match(first.failedModules[0].bodyUnavailable, /cannot be associated/);
  // One capture whose status differs from the script response: unavailable too.
  const mismatched = createFeedbackDiagnostics();
  mismatched.install({ middlewares: { use(fn) { middleware = fn; } } });
  serve(404, "not this one");
  mismatched.response({ url: () => origin + "/src/lazy.js", status: () => 500, headers: () => ({}),
    request: () => ({ resourceType: () => "script" }), body: () => assert.fail("unbounded body acquisition is forbidden") }, origin);
  const rows = await mismatched.finish(); mismatched.dispose();
  assert.equal(rows.failedModules[0].body, undefined);
  assert.match(rows.failedModules[0].bodyUnavailable, /cannot be associated/);
});
test("a never-finishing 500 response times out without calling Playwright body or blocking teardown", async () => {
  const diagnostics = createFeedbackDiagnostics({ timeoutMs: 15 });
  diagnosticResponse(diagnostics, "http://app.test", "/never.ts").write("partial");
  const result = await diagnostics.finish(); diagnostics.dispose();
  assert.equal(result.failedModules[0].body.text, "partial");
  assert.equal(result.failedModules[0].body.timedOut, true); assert.equal(result.failedModules[0].body.incomplete, true);
});
test("bounded outgoing capture preserves buffer chunks and callback-only end overloads", async () => {
  const diagnostics = createFeedbackDiagnostics();
  const response = diagnosticResponse(diagnostics, "http://app.test", "/buffers.ts");
  assert.equal(response.write(Buffer.from("日本語")), true);
  response.write(new Uint8Array(Buffer.from(" error")));
  response.end(() => {});
  const result = await diagnostics.finish(); diagnostics.dispose();
  assert.equal(result.failedModules[0].body.text, "日本語 error");
  assert.equal(result.failedModules[0].body.bytes, Buffer.byteLength("日本語 error"));
});
test("missing bounded response capture is explicit and never buffers a browser response", async () => {
  const diagnostics = createFeedbackDiagnostics();
  diagnostics.response({ url: () => "http://app.test/a.ts", status: () => 500, headers: () => ({}),
    request: () => ({ resourceType: () => "script" }), body: () => assert.fail() }, "http://app.test");
  assert.match((await diagnostics.finish()).failedModules[0].bodyUnavailable, /No bounded/);
});
test("hanging resource close is bounded and does not block later close attempts or cache deletion", async () => {
  const cache = mkdtempSync(join(tmpdir(), "solid-feedback-hanging-close-")), calls = [];
  await assert.rejects(closeFeedbackResources({ cache, timeoutMs: 15,
    context: { close: () => new Promise(() => { calls.push("context"); }) },
    server: { close: async () => calls.push("server") }, browser: { close: async () => calls.push("browser") }
  }), /context close timed out/);
  assert.deepEqual(calls, ["context", "server", "browser"]); assert.equal(existsSync(cache), false);
});

test("a cleanup failure after a primary failure is attached to it rather than replacing it", async () => {
  const failing = { close: async () => { throw new Error("close failed"); } };
  const primary = new Error("native failed");
  await closeFeedbackResourcesAfter(primary, { browser: failing });
  assert.equal(primary.cleanupErrors.length, 1);
  await assert.rejects(closeFeedbackResourcesAfter(undefined, { browser: failing }));
});

test("a dropped server capture disables body association for every failed script", async () => {
  const origin = "http://app.test", diagnostics = createFeedbackDiagnostics();
  let middleware;
  diagnostics.install({ middlewares: { use(fn) { middleware = fn; } } });
  const serve = (url, text) => {
    const response = Object.assign(new EventEmitter(), { statusCode: 500, writableFinished: false,
      writeHead() {}, write() { return true; }, end() { this.writableFinished = true; this.emit("finish"); } });
    middleware({ url }, response, () => {});
    response.writeHead(500); response.end(text);
  };
  serve("/src/lazy.js", '{"error":"fetch"}');
  for (let i = 0; i < 63; i++) serve(`/api/${i}`, "fetch failed");
  serve("/src/lazy.js", "compiler error");
  diagnostics.response({ url: () => origin + "/src/lazy.js", status: () => 500, headers: () => ({}),
    request: () => ({ resourceType: () => "script" }), body: () => assert.fail("unbounded body acquisition is forbidden") }, origin);
  const result = await diagnostics.finish(); diagnostics.dispose();
  assert.equal(result.capturesDropped, 1);
  assert.equal(result.failedModules[0].body, undefined);
  assert.match(result.failedModules[0].bodyUnavailable, /dropped/);
});

test("a server.proxy bypass function vetoes the run: it can serve a module URL as another module", () => {
  validateFeedbackRequestRouting({});
  validateFeedbackRequestRouting({ proxy: { "/api": "http://127.0.0.1:1", "/rpc": { target: "http://127.0.0.1:1" } } });
  assert.throws(() => validateFeedbackRequestRouting({ proxy: { "/src/a.js": { target: "http://127.0.0.1:1", bypass: () => "/src/b.js" } } }),
    /server\.proxy\["\/src\/a\.js"\]\.bypass/);
  // A function-valued or array-valued option carries its bypass just the same.
  const fn = Object.assign(function proxyOptions() {}, { target: "http://127.0.0.1:1", bypass: () => "/src/b.js" });
  assert.throws(() => validateFeedbackRequestRouting({ proxy: { "/src/a.js": fn } }), /bypass/);
  const list = Object.assign([], { bypass: () => "/src/b.js" });
  assert.throws(() => validateFeedbackRequestRouting({ proxy: { "/src/a.js": list } }), /bypass/);
});
