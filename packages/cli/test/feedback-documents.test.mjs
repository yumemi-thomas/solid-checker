import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import { test } from "vitest";
import { observeFeedbackDocuments } from "../scripts/feedback-documents.mjs";

const entry = "http://app.test/entry.js", entries = [{ url: entry }], final = { id: "last" };
async function harness(options, body = async () => ({ body: "selected", base64Encoded: false })) {
  const cdp = new EventEmitter();
  cdp.send = async (method, params) => {
    if (method === "Page.getFrameTree") return { frameTree: { frame: { id: "top" } } };
    if (method === "Network.getResponseBody") return body(params.requestId);
  };
  const observer = await observeFeedbackDocuments(cdp, options);
  const navigate = (loaderId, id, { frameId = "top", parentId, bind = true, requestId = loaderId } = {}) => {
    cdp.emit("Page.frameNavigated", { frame: { id: frameId, parentId, loaderId, url: "http://app.test/" } });
    const executionContextId = loaderId.length + (frameId === "top" ? 0 : 100);
    cdp.emit("Runtime.executionContextCreated", { context: { id: executionContextId, auxData: { frameId, isDefault: true } } });
    if (bind) cdp.emit("Runtime.bindingCalled", { name: "__solidCheckerFlush", executionContextId,
      payload: JSON.stringify({ phase: "start", id }) });
    cdp.emit("Network.responseReceived", { type: "Document", requestId, frameId, loaderId,
      response: { url: "http://app.test/", status: 200, mimeType: "text/html" } });
    cdp.emit("Network.loadingFinished", { requestId, encodedDataLength: 16 });
  };
  // Explicit inert-parser boundary double: only the original response body
  // controls the entries; a self-removing script cannot alter that body.
  const parsed = [], page = { evaluate: async (_parse, { html }) => {
    parsed.push(html);
    return [{ url: entry }, ...(html.includes("extra-inline") ? [{ url: null, inline: "document.currentScript.remove()" }]
      : html.includes("extra-external") ? [{ url: "http://app.test/removes-itself.js" }] : [])];
  } };
  return { cdp, observer, navigate, page, parsed };
}

for (const extra of ["extra-inline", "extra-external"]) test(`served ${extra} cannot disappear from entry proof by removing its final DOM element`, async () => {
  const h = await harness({}, async () => ({ body: extra, base64Encoded: false }));
  h.navigate("loader", "last");
  await assert.rejects(h.observer.finish(h.page, entries, [], { ...final, modules: [entry] }), /unselected module entry/);
  assert.deepEqual(h.parsed, [extra]);
});

test("every earlier served document is checked even after navigating to a valid final shell", async () => {
  const h = await harness({}, async requestId => ({ body: requestId === "earlier" ? "extra-external" : "selected" }));
  h.navigate("earlier", "first"); h.navigate("last", "last");
  await assert.rejects(h.observer.finish(h.page, entries, [], final), /unselected module entry/);
  assert.deepEqual(h.parsed, ["extra-external"]);
});

test("an iframe start cannot offset an unbound top-level loader", async () => {
  const h = await harness();
  h.navigate("unbound", "absent", { bind: false });
  h.navigate("iframe", "iframe-bound", { frameId: "child", parentId: "top" });
  h.navigate("last", "last");
  assert.equal(h.observer.messages.length, 1);
  await assert.rejects(h.observer.finish(h.page, entries, [], final), /own binding/);
});

test("response pins and bootstrap identities belong to exact top-level frame/loader/contexts", async () => {
  const h = await harness();
  h.navigate("earlier", "first"); h.navigate("last", "last");
  const result = await h.observer.finish(h.page, entries, [], final);
  assert.deepEqual(result.map(row => [row.frameId, row.loaderId, row.binding.id, row.binding.executionContextId]),
    [["top", "earlier", "first", 7], ["top", "last", "last", 4]]);
  assert(result.every(row => row.sha256.startsWith("sha256:") && row.bytes === 8));
  await assert.rejects(h.observer.finish(h.page, entries, [], { id: "first" }), /Final document bootstrap/);
});

for (const [name, options, body, expected] of [
  ["unavailable", {}, async () => { throw Error("No resource with given identifier"); }, /body unavailable/],
  ["timeout", { timeoutMs: 5 }, () => new Promise(() => {}), /timed out/],
  ["oversize", { maxBodyBytes: 20 }, async () => ({ body: "x".repeat(21) }), /byte budget/],
  ["total", { maxTotalBytes: 128 }, async () => ({ body: "x".repeat(129) }), /byte budget/]
]) test(`served document body ${name} refuses explicitly within bounded acquisition`, async () => {
  const h = await harness(options, body); h.navigate("loader", "last");
  await assert.rejects(h.observer.finish(h.page, entries, [], final), expected);
});

test("a loader without a network response refuses instead of using its DOM", async () => {
  const h = await harness();
  h.cdp.emit("Page.frameNavigated", { frame: { id: "top", loaderId: "no-response", url: "http://app.test/" } });
  await assert.rejects(h.observer.finish(h.page, entries, [], final), /body could not be obtained exactly/);
});
