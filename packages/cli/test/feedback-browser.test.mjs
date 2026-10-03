import assert from "node:assert/strict";
import { test } from "vitest";
import { createReadCollector } from "../scripts/feedback-read-runtime.mjs";
import { selectAssertionFeedback } from "../scripts/feedback-assertion-selector.mjs";
import { executeFeedbackScenario, loadFeedbackConfiguration, mergeFeedbackServerConfiguration, validateFeedbackScenario } from "../scripts/feedback-browser.mjs";
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
