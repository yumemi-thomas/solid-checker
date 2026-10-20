import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { feedbackHtmlEntries, validateFeedbackServedEntries } from "./feedback-entry-bootstrap.mjs";

// The response body is execution history; the application's final DOM is not.
// Keep only top-level documents and default-world bindings for their loaders.
export async function observeFeedbackDocuments(cdp, { timeoutMs = 2000, maxBodyBytes = 1024 * 1024,
  maxTotalBytes = 8 * 1024 * 1024, maxDocuments = 64 } = {}) {
  const documents = new Map(), requests = new Map(), contexts = new Map(), pending = new Set(), messages = [];
  let topFrame, currentLoader, totalBytes = 0, failure;
  const refuse = error => { failure ??= error; };
  cdp.on("Page.frameNavigated", ({ frame }) => {
    if (frame.parentId || frame.url === "about:blank") return;
    topFrame = frame.id; currentLoader = frame.loaderId;
    if (!documents.has(currentLoader)) {
      if (documents.size >= maxDocuments) { refuse(new Error("Served document observation exceeded its document budget")); return; }
      documents.set(currentLoader, { frameId: frame.id, loaderId: currentLoader, url: frame.url });
    }
  });
  cdp.on("Runtime.executionContextCreated", ({ context }) => {
    if (!context.auxData?.isDefault) return;
    if (contexts.size >= maxDocuments * 8) { refuse(new Error("Document binding context observation exceeded its budget")); return; }
    contexts.set(context.id, { frameId: context.auxData.frameId,
      loaderId: context.auxData.frameId === topFrame ? currentLoader : null });
  });
  cdp.on("Runtime.bindingCalled", ({ name, payload, executionContextId }) => {
    if (name !== "__solidCheckerFlush") return;
    const context = contexts.get(executionContextId);
    if (!context) { refuse(new Error("Document bootstrap binding has no observed execution context")); return; }
    if (context.frameId !== topFrame || !context.loaderId) return; // Iframes cannot satisfy a top-level binding.
    if (messages.length >= maxDocuments * 2 || Buffer.byteLength(payload) > maxTotalBytes) {
      refuse(new Error("Document binding observation exceeded its budget")); return;
    }
    try { messages.push({ ...JSON.parse(payload), ...context, executionContextId }); }
    catch { refuse(new Error("Malformed document bootstrap binding")); }
  });
  cdp.on("Network.responseReceived", ({ type, requestId, frameId, loaderId, response }) => {
    if (type !== "Document" || frameId !== topFrame) return;
    if (requests.size >= maxDocuments) { refuse(new Error("Served document response observation exceeded its budget")); return; }
    requests.set(requestId, { frameId, loaderId, url: response.url, status: response.status, mimeType: response.mimeType });
  });
  cdp.on("Network.loadingFailed", ({ requestId }) => {
    if (requests.has(requestId)) refuse(new Error("Served top-level document response could not be obtained"));
  });
  cdp.on("Network.loadingFinished", ({ requestId, encodedDataLength }) => {
    const row = requests.get(requestId);
    if (!row) return;
    const task = (async () => {
      let timer;
      try {
        assert(encodedDataLength <= maxBodyBytes, "Served document response exceeded its byte budget");
        assert(row.status >= 200 && row.status < 300 && row.mimeType === "text/html",
          "Served top-level document was not a successful HTML response");
        const result = await Promise.race([cdp.send("Network.getResponseBody", { requestId }), new Promise((_, reject) => {
          timer = setTimeout(() => reject(new Error("Served document response body acquisition timed out")), timeoutMs);
        })]);
        const bytes = Buffer.from(result.body, result.base64Encoded ? "base64" : "utf8");
        totalBytes += bytes.length;
        assert(bytes.length <= maxBodyBytes && totalBytes <= maxTotalBytes, "Served document response exceeded its byte budget");
        row.html = bytes.toString("utf8"); row.bytes = bytes.length;
        row.sha256 = "sha256:" + createHash("sha256").update(bytes).digest("hex");
      } catch (error) { refuse(new Error(`Served document response body unavailable: ${error.message}`)); }
      finally { clearTimeout(timer); }
    })();
    pending.add(task); task.finally(() => pending.delete(task));
  });
  await cdp.send("Page.enable");
  const tree = await cdp.send("Page.getFrameTree");
  topFrame = tree.frameTree.frame.id;
  await cdp.send("Runtime.enable");
  await cdp.send("Runtime.addBinding", { name: "__solidCheckerFlush" });
  await cdp.send("Network.enable", { maxTotalBufferSize: maxTotalBytes, maxResourceBufferSize: maxBodyBytes });
  return {
    messages,
    async finish(page, entries, auxiliary, final) {
      await Promise.all([...pending]);
      if (failure) throw failure;
      assert(documents.size, "Browser feedback did not observe a top-level document loader");
      const served = [];
      for (const document of documents.values()) {
        const responses = [...requests.values()].filter(row => row.loaderId === document.loaderId && row.frameId === document.frameId);
        assert(responses.length === 1 && typeof responses[0].html === "string",
          "Served top-level document response body could not be obtained exactly");
        const response = responses[0];
        const rows = await page.evaluate(feedbackHtmlEntries, { html: response.html, url: response.url });
        validateFeedbackServedEntries(rows.map(row => row.url), entries, auxiliary);
        const starts = messages.filter(row => row.phase === "start" && row.frameId === document.frameId && row.loaderId === document.loaderId);
        assert(starts.length === 1 && starts[0].id, "Browser feedback bootstrap missed a navigated document's own binding");
        served.push({ ...document, url: response.url, bytes: response.bytes, sha256: response.sha256,
          entries: rows, binding: { id: starts[0].id, executionContextId: starts[0].executionContextId } });
      }
      assert(messages.some(row => row.phase === "start" && row.id === final?.id && row.loaderId === currentLoader && row.frameId === topFrame),
        "Final document bootstrap does not match its top-level loader binding");
      return served;
    }
  };
}
