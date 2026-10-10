import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { afterAll, test } from "vitest";
import { classifyUnmappedFrames, createFrameAttributor, summarizeCandidateScopes, summarizeUnmapped } from "../scripts/feedback-attribution.mjs";
import { createReadCollector } from "../scripts/feedback-read-runtime.mjs";

const root = realpathSync(mkdtempSync(join(tmpdir(), "solid-feedback-attribution-")));
afterAll(() => rmSync(root, { recursive: true, force: true }));
const write = (path, text) => { mkdirSync(join(root, path, ".."), { recursive: true }); writeFileSync(join(root, path), text); };
write("package.json", JSON.stringify({ name: "app", version: "0.0.0" }));
write("src/App.tsx", "export const value = 1;\n");
write("src/generated.js", "export const other = 2;\n");
write("node_modules/@solidjs/signals/package.json", JSON.stringify({ name: "@solidjs/signals", version: "2.0.0-rc.9" }));
write("node_modules/@solidjs/signals/dist/dev-shared.js", "read();\n");
write("node_modules/@solidjs/router/package.json", JSON.stringify({ name: "@solidjs/router", version: "0.16.0" }));
write("node_modules/@solidjs/router/dist/package.json", JSON.stringify({ type: "module" }));
write("node_modules/@solidjs/router/dist/routing.js", "useLocation();\n");
write("collector.mjs", "");

const origin = "http://127.0.0.1:5173";
const sourceFiles = new Map([[join(root, "src/App.tsx"), { text: "export const value = 1;\n" }]]);
const maps = { "/src/App.tsx": { source: "App.tsx" }, "/src/generated.js": { source: "generated.ts" },
  "/node_modules/@solidjs/router/dist/routing.js": { source: "../src/routing.ts" } };
const attribute = createFrameAttributor({ origin, root, sourceFiles, collectorPaths: [join(root, "collector.mjs")],
  mapFor: async key => maps[key] ?? null,
  originalPositionFor: (map, { line, column }) => ({ source: map.source, line, column }) });
const frame = (path, line = 1, column = 1) => ({ path: `${origin}${path}`, line, column });
const reader = frame("/node_modules/@solidjs/signals/dist/dev-shared.js"), router = frame("/node_modules/@solidjs/router/dist/routing.js");

test("every frame keeps the outcome that stopped its attribution", async () => {
  assert.equal((await attribute({ path: "native", line: 1, column: 1 })).outcome, "not-a-url");
  assert.equal((await attribute({ path: "http://elsewhere.test/a.js", line: 1, column: 1 })).outcome, "foreign-origin");
  assert.equal((await attribute(frame("/@id/virtual:entry"))).outcome, "virtual-module");
  assert.equal((await attribute(frame("/src/missing.ts"))).outcome, "served-path-missing");
  assert.equal((await attribute(frame(`/@fs${root}/collector.mjs`))).outcome, "collector");
  const signals = await attribute(reader);
  assert.equal(signals.outcome, "no-source-map");
  assert.deepEqual(signals.package, { name: "@solidjs/signals", version: "2.0.0-rc.9", installed: true });
  // A nameless nested package.json does not hide the installed package.
  const routing = await attribute(router);
  assert.equal(routing.outcome, "outside-configured-sources"); assert.equal(routing.package.name, "@solidjs/router");
  assert.equal(routing.original.path, join(root, "node_modules/@solidjs/router/src/routing.ts"));
  assert.equal((await attribute(frame("/src/App.tsx", 9))).outcome, "position-out-of-range");
  const mapped = await attribute(frame("/src/App.tsx", 1, 3));
  assert.equal(mapped.outcome, "mapped"); assert.equal(mapped.package.installed, false);
  assert.deepEqual(mapped.original, { path: join(root, "src/App.tsx"), line: 1, column: 3 });
});

test("a complete stack of installed package frames is package-only and names the first non-reader frame", async () => {
  const frames = [reader, router, reader];
  const row = classifyUnmappedFrames(frames, await Promise.all(frames.map(attribute)), { stackTruncated: false });
  assert.equal(row.attribution, "package-frames-only");
  assert.deepEqual(row.packages.map(pkg => pkg.name), ["@solidjs/signals", "@solidjs/router"]);
  assert.equal(row.firstPackageFrame.package.name, "@solidjs/router");
  assert.equal(row.frames[0].package, "@solidjs/signals@2.0.0-rc.9");
  assert.match(row.reason, /Responsibility remains open/);
  assert(!JSON.stringify(row).includes("\"source\""), "Source text is not copied into the record");
});

for (const stackTruncated of [true, null]) {
  test(`a package-only stack whose completeness is ${stackTruncated === null ? "unknown" : "truncated"} stays incomplete`, async () => {
    const row = classifyUnmappedFrames([reader, router], [await attribute(reader), await attribute(router)], { stackTruncated });
    assert.equal(row.attribution, "stack-incomplete");
  });
}

test("an application-owned frame that fails to map is lost attribution, not package code", async () => {
  const frames = [reader, frame("/src/generated.js"), router];
  const row = classifyUnmappedFrames(frames, await Promise.all(frames.map(attribute)), { stackTruncated: false });
  assert.equal(row.attribution, "application-frame-unmapped");
  assert.equal(row.frames[1].outcome, "outside-configured-sources"); assert.equal(row.frames[1].installed, false);
});

test("frames outside the application server are not attributable, and the collector's own frames are ignored", async () => {
  const frames = [frame(`/@fs${root}/collector.mjs`), { path: "http://elsewhere.test/a.js", line: 1, column: 1 }];
  const row = classifyUnmappedFrames(frames, await Promise.all(frames.map(attribute)), { stackTruncated: false });
  assert.equal(row.attribution, "no-attributable-frame"); assert.equal(row.firstPackageFrame, null);
  assert.deepEqual(summarizeUnmapped([row, row, { attribution: "package-frames-only" }]),
    { total: 3, byAttribution: { "no-attributable-frame": 2, "package-frames-only": 1 } });
});

// The collector runs in Chromium, so stack capture is checked under V8 (Node)
// rather than the suite's own runtime, whose stack semantics differ.
test("collector stacks report reaching their capture depth and restore the page's limit", () => {
  const script = `import { createReadCollector } from ${JSON.stringify(pathToFileURL(fileURLToPath(new URL("../scripts/feedback-read-runtime.mjs", import.meta.url))).href)};
    const limit = Error.stackTraceLimit, nest = (depth, run) => depth ? nest(depth - 1, run) : run();
    const shallow = createReadCollector().begin({}, () => null, () => null);
    const deep = nest(5, () => createReadCollector({ maxFrames: 2 }).begin({}, () => null, () => null));
    console.log(JSON.stringify({ shallow: shallow.stackTruncated, shallowFrames: shallow.frames.length,
      deep: deep.stackTruncated, deepFrames: deep.frames.length, restored: Error.stackTraceLimit === limit }));`;
  const result = JSON.parse(execFileSync("node", ["--input-type=module", "-e", script], { encoding: "utf8" }));
  assert.deepEqual({ ...result, shallowFrames: result.shallowFrames > 0 },
    { shallow: false, shallowFrames: true, deep: true, deepFrames: 2, restored: true });
});

const span = (start, end) => ({ start, end }), path = "/consumer.ts";
const fn = { path, sourceSha256: "sha256:test", span: span(0, 100), derivedOrigin: span(10, 90) };
const op = { path, sourceSha256: "sha256:test", span: span(30, 40), function: span(0, 100), resultRelevance: "return-expression" };

test("collector counts entered candidate scopes and synchronous operation completion", () => {
  const collector = createReadCollector(), encoded = JSON.stringify(fn), operation = JSON.stringify(op);
  const token = collector.enterFunction(encoded, null); collector.enterFunction(encoded, null);
  assert.equal(collector.withOperation(token, operation, () => 7), 7);
  assert.throws(() => collector.withOperation(token, operation, () => { throw new Error("boom"); }), /boom/);
  const rows = collector.scopes.rows;
  assert.deepEqual(rows.find(row => row.kind === "function"), { kind: "function", path, sourceSha256: "sha256:test", span: fn.span, entered: 2 });
  assert.deepEqual(rows.find(row => row.kind === "operation"), { kind: "operation", path, sourceSha256: "sha256:test", span: op.span, entered: 2, returned: 1, threw: 1 });
  const bounded = createReadCollector({ maxScopes: 1 });
  bounded.enterFunction(encoded, null); bounded.withOperation(null, operation, () => 0);
  assert.equal(bounded.scopes.rows.length, 1); assert.equal(bounded.scopes.dropped, 1);
});

test("candidate scope summary separates unloaded files, skipped instrumentation and unexecuted scopes", () => {
  const model = { path, sourceSha256: "sha256:test",
    functions: [{ span: fn.span, derivedOrigin: fn.derivedOrigin }, { span: span(50, 60), derivedOrigin: span(51, 59) }, { span: span(70, 80) }],
    operations: [op, { span: span(41, 45), function: span(0, 100), resultRelevance: "open" }, { span: span(52, 55), function: span(50, 60), resultRelevance: "local-initializer" }] };
  const other = { path: "/unloaded.ts", sourceSha256: "sha256:other", functions: [{ span: span(0, 5), derivedOrigin: span(1, 4) }], operations: [] };
  const instrumented = new Map([[path, { functions: new Set(["0:100", "50:60"]), operations: new Set(["30:40"]) }]]);
  const scopes = { rows: [{ kind: "function", path, span: fn.span, entered: 1 }, { kind: "operation", path, span: op.span, entered: 1, returned: 1, threw: 0 }], dropped: 0 };
  const summary = summarizeCandidateScopes([model, other], { instrumented, scopes });
  assert.deepEqual(summary.files, { modeled: 2, loaded: 1 });
  assert.deepEqual(summary.derivedOrigins, { modeled: 3, instrumented: 2, entered: 1 });
  assert.deepEqual(summary.resultOperations, { modeled: 2, instrumented: 1, entered: 1, returned: 1, threw: 0 });
  assert.deepEqual(summary.notEntered.map(row => row.status).sort(), ["file-not-loaded", "not-entered", "not-instrumented"]);
  assert.equal(summary.asyncSettlement, "unobserved"); assert.equal(summary.complete, false);
  assert.equal(summarizeCandidateScopes([model], { instrumented, scopes, limit: 1 }).notEnteredDropped, 1);
  const located = summarizeCandidateScopes([model], { instrumented, scopes, lineOf: (file, byte) => `${file}@${byte}` });
  assert.equal(located.notEntered.find(row => row.status === "not-entered").line, `${path}@50`);
});

test("Solid dev diagnostics keep their code, message and a bounded stack", () => {
  const collector = createReadCollector({ maxDiagnostics: 1 });
  collector.diagnostic({ code: "NO_OWNER_CLEANUP", kind: "lifecycle", severity: "warn", message: "x".repeat(600) });
  collector.diagnostic({ code: "STRICT_READ_UNTRACKED", kind: "strict-read", severity: "warn", message: "second" });
  assert.equal(collector.diagnostics.length, 1); assert.equal(collector.diagnosticsDropped, 1);
  const [row] = collector.diagnostics;
  assert.equal(row.code, "NO_OWNER_CLEANUP"); assert.equal(row.message.length, 400);
  assert(Array.isArray(row.frames)); assert.equal(typeof row.stackTruncated, "boolean");
});

test("a diagnosed operation is attributed past Solid's runtime to the code that performed it", async () => {
  const { operationFrame } = await import("../scripts/feedback-attribution.mjs");
  const signals = await attribute(reader), routing = await attribute(router), app = await attribute(frame("/src/App.tsx", 1, 3));
  assert.deepEqual(operationFrame([signals, routing, app]), { in: "package", package: { name: "@solidjs/router", version: "0.16.0" } });
  assert.deepEqual(operationFrame([signals, app]), { in: "application" });
  assert.deepEqual(operationFrame([await attribute(frame(`/@fs${root}/collector.mjs`)), signals]), { in: "unknown" });
});

test("a diagnosed site is a package export's own call or an access to a value", async () => {
  const { siteExpressionKind } = await import("../scripts/feedback-attribution.mjs");
  const text = 'import { createPolled } from "@solid-primitives/timer";\nimport * as T from "@solid-primitives/timer";\nimport { local } from "./local";\nconst now = createPolled(() => 1, 10);\nconst a = now();\nconst b = position.y;\nconst c = T.createPolled(() => 1, 10);\nconst d = local();\n';
  const at = needle => text.indexOf(needle);
  assert.equal(siteExpressionKind(text, "/a.tsx", at("createPolled(() =>")), "package-call");
  assert.equal(siteExpressionKind(text, "/a.tsx", at("now()")), "value-access");
  assert.equal(siteExpressionKind(text, "/a.tsx", at("y;")), "value-access");
  assert.equal(siteExpressionKind(text, "/a.tsx", at("T.createPolled") + 2), "package-call");
  assert.equal(siteExpressionKind(text, "/a.tsx", at("local()")), "value-access");
});
