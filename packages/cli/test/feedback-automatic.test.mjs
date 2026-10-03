import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { test } from "vitest";
import { createReadCollector } from "../scripts/feedback-read-runtime.mjs";
import { selectReadFeedback } from "../scripts/feedback-read-selector.mjs";
import { instrumentFeedbackSource } from "../scripts/feedback-source-hook.mjs";

const span = (start, end) => ({ start, end }), path = "/consumer.ts", sourceSha256 = "sha256:test";
const model = { path, sourceSha256, functions: [{ span: span(0, 100), derivedOrigin: span(10, 90), asynchronous: true }],
  operations: [{ span: span(30, 40), function: span(0, 100), resultRelevance: "return-expression" }] };
const event = { nodeId: 1, site: { sourceSha256, location: { path, startByte: 31, endByte: 32 } }, lineage: { origin: { path, sourceSha256, span: span(10, 90) },
  steps: [{ path, sourceSha256, ...model.operations[0] }], truncated: false } };
test("automatic selection joins current native return models without assertions or comparison code", () => {
  const result = selectReadFeedback([event, event], [model]);
  assert.equal(result.notes.length, 1); assert.equal(result.notes[0].severity, "warning");
  assert.equal(result.notes[0].reactiveIntent, "open"); assert.equal(result.notes[0].authority, false);
});
for (const changed of [ { ...event, lineage: null },
  { ...event, lineage: { ...event.lineage, truncated: true } },
  { ...event, lineage: { ...event.lineage, steps: null } },
  { ...event, site: { sourceSha256, location: { path, startByte: 80, endByte: 81 } } },
  { ...event, lineage: { ...event.lineage, origin: { ...event.lineage.origin, sourceSha256: "stale" } } },
  { ...event, lineage: { ...event.lineage, steps: [{ ...event.lineage.steps[0], span: span(31, 40) }] } } ]) {
  test(`missing or stale exact evidence stays open: ${JSON.stringify(changed)}`, () => {
    const result = selectReadFeedback([changed], [model]); assert.equal(result.notes.length, 0); assert(result.open.length);
  });
}
test("discarded read results stay open, and a plain deferred callback can produce guidance", () => {
  assert.equal(selectReadFeedback([event], [{ ...model, operations: [{ ...model.operations[0], resultRelevance: "open" }] }]).notes.length, 0);
  assert.equal(selectReadFeedback([event], [{ ...model, functions: [{ ...model.functions[0], asynchronous: false }] }]).notes.length, 1);
});
test("late callback allocations must relate to the parent's result", () => {
  const child = { span: span(60, 80), parent: span(0, 100), allocationRelevance: "open" };
  const next = { ...event, lineage: { ...event.lineage, steps: [...event.lineage.steps,
    { kind: "allocation", path, sourceSha256, span: child.span, function: child.parent }, event.lineage.steps[0]] } };
  assert.equal(selectReadFeedback([next], [{ ...model, functions: [...model.functions, child] }]).notes.length, 0);
  assert.equal(selectReadFeedback([next], [{ ...model, functions: [...model.functions, { ...child, allocationRelevance: "return-expression" }] }]).notes.length, 1);
});
test("observer queries remain informational and do not duplicate an observed-read warning", () => {
  const query = { ...event, kind: "observer-query", nodeId: null };
  const result = selectReadFeedback([query], [model]); assert.equal(result.notes.length, 1); assert.equal(result.notes[0].severity, "info");
  assert.equal(result.notes[0].reactiveRead, "unproven");
  assert.equal(selectReadFeedback([query, event], [model]).notes.length, 1);
});
test("query collection preserves values, excludes the collector's own query, and respects explicit untrack", () => {
  const collector = createReadCollector(), token = collector.enterFunction(JSON.stringify({ path, sourceSha256, derivedOrigin: span(10, 90) }), null);
  collector.withOperation(token, "{}", () => {
    assert.equal(collector.observerQuery(null, () => null), null);
    const value = {}; assert.equal(collector.observerQuery(value, () => null), value);
    collector.finish(1, collector.begin({}, () => collector.observerQuery(null, () => null), () => null));
    const previous = collector.enterIntent(); collector.observerQuery(null, () => null); collector.leaveIntent(previous);
  });
  assert.deepEqual(collector.events.map(row => row.kind), ["observer-query", "untracked-read"]);
});
test("runtime-supplied return flags cannot override the native model", () => {
  const changed = { ...model, operations: [{ ...model.operations[0], resultRelevance: "open" }] };
  assert.equal(selectReadFeedback([event], [changed]).notes.length, 0);
});
test("native local-initializer and distinct-branch candidates are accepted without invented API semantics", () => {
  for (const resultRelevance of ["local-initializer", "distinct-branch"]) {
    assert.equal(selectReadFeedback([event], [{ ...model, operations: [{ ...model.operations[0], resultRelevance }] }]).notes.length, 1);
  }
});
test("published typing errors suppress automatic warnings and retention losses stay visible", () => {
  assert.equal(selectReadFeedback([event], [model], { typingErrors: 1 }).notes.length, 0);
  assert.match(selectReadFeedback([], [model], { dropped: 3 }).open[0].reason, /retention/);
});
test("async function tokens preserve origin across continuation, restore callers, and bound lineage", async () => {
  const collector = createReadCollector(), node = {};
  const token = collector.enterFunction(JSON.stringify({ path, sourceSha256, derivedOrigin: span(10, 90) }), null);
  await Promise.resolve();
  collector.withOperation(token, JSON.stringify({ path, sourceSha256, ...model.operations[0] }), () => {
    const ticket = collector.begin(node, () => null, () => null); collector.finish(1, ticket);
  });
  assert.deepEqual(collector.events[0].lineage.origin, event.lineage.origin);
  assert.equal(collector.begin(node, () => null, () => null).lineage, null);
  let nested = token;
  for (let i = 0; i < 35; i++) collector.withOperation(nested, "{}", () => { nested = collector.enterFunction("{}", null); });
  collector.withOperation(nested, "{}", () => { assert(collector.begin(node, () => null, () => null).lineage.truncated); });
  assert.throws(() => collector.withOperation(token, "{}", () => { throw Error("failure"); }));
  assert.equal(collector.begin(node, () => null, () => null).lineage, null);
});
test("source transformation preserves async return values, receiver calls and UTF8 identities", async () => {
  const text = 'const label="é"; export async function example(){"use strict"; await Promise.resolve(); return receiver.get();}';
  const start = Buffer.byteLength(text.slice(0, text.indexOf("export async"))), end = Buffer.byteLength(text);
  const body = Buffer.byteLength(text.slice(0, text.indexOf('{'))), callStart = Buffer.byteLength(text.slice(0, text.indexOf("receiver.get()")));
  const sourceModel = { path, sourceSha256: `sha256:${createHash("sha256").update(text).digest("hex")}`,
    functions: [{ span: span(start, end), body: span(body, end), parent: null, asynchronous: true, generator: false, derivedOrigin: span(start, end) }],
    operations: [{ span: span(callStart, callStart + 14), function: span(start, end), resultRelevance: "return-expression" }] };
  const transformed = instrumentFeedbackSource(text, path, sourceModel, "/runtime.js");
  assert.equal(transformed.counts.functions, 1); assert.equal(transformed.counts.operations, 1);
  const value = {}, receiver = { get() { assert.equal(this, receiver); return value; } }, collector = createReadCollector();
  const code = transformed.code.replace(/^import .*;\n/, "").replace("export async function", "async function");
  const fn = new Function("__scDevelopmentRuntime", "receiver", code + "\nreturn example;")(collector, receiver);
  assert.equal(await fn(), value);
  assert.throws(() => instrumentFeedbackSource(text + " ", path, sourceModel, "/runtime.js"), /source changed/);
});
test("method models join by exact body span and preserve receiver and method name", () => {
  const text = "const receiver={get(){return this;}};";
  const bodyStart = text.indexOf("{return"), end = text.indexOf("}}") + 1;
  const sourceModel = { path, sourceSha256: `sha256:${createHash("sha256").update(text).digest("hex")}`,
    functions: [{ span: span(text.indexOf("(){"), end), body: span(bodyStart, end), parent: null, generator: false }], operations: [] };
  const result = instrumentFeedbackSource(text, path, sourceModel, "/runtime.js");
  assert.equal(result.counts.functions, 1);
  const code = result.code.replace(/^import .*;\n/, "");
  const receiver = new Function("__scDevelopmentRuntime", code + "\nreturn receiver;")(createReadCollector());
  assert.equal(receiver.get(), receiver); assert.equal(receiver.get.name, "get");
});
