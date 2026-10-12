import assert from "node:assert/strict";
import { createServer } from "node:http";
import { createRequire } from "node:module";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { test } from "vitest";
import { observeFeedbackDocuments } from "../scripts/feedback-documents.mjs";

// Opt in with an existing installation/executable. Never install a browser or
// fetch tooling as part of this test. The CDP unit suite runs unconditionally.
const tooling = process.env.SOLID_CHECKER_FEEDBACK_TEST_TOOLING;
const executablePath = process.env.SOLID_CHECKER_FEEDBACK_TEST_BROWSER;
for (const kind of ["removed-inline", "removed-external", "earlier-shell", "iframe-offset"])
  test.skipIf(!tooling || !executablePath)(`CDP served-document proof rejects ${kind} after actual browser execution`, async () => {
    const require = createRequire(join(tooling, "package.json"));
    const playwright = await import(pathToFileURL(require.resolve("playwright")));
    const chromium = playwright.chromium ?? playwright.default.chromium;
    const selected = '<script type="module" src="/entry.js"></script>';
    const extraInline = '<script id="extra" type="module">document.getElementById("extra").remove(); globalThis.__extraExecuted=true;</script>';
    const extraExternal = '<script id="extra" type="module" src="/extra.js"></script>';
    const server = createServer((request, response) => {
      if (request.url === "/entry.js" || request.url === "/extra.js") {
        response.setHeader("Content-Type", "text/javascript");
        response.end(request.url === "/extra.js" ? 'document.getElementById("extra").remove(); globalThis.__extraExecuted=true;'
          : `if(location.pathname!=='/unbound') {
            globalThis.__solidCheckerDocument=crypto.randomUUID();
            globalThis.__solidCheckerFlush(JSON.stringify({phase:'start',id:globalThis.__solidCheckerDocument})); }`);
        return;
      }
      response.setHeader("Content-Type", "text/html");
      const extra = request.url === "/first" ? extraInline : request.url !== "/" ? ""
        : kind === "removed-inline" ? extraInline : kind === "removed-external" ? extraExternal : "";
      response.end(selected + extra + (request.url === "/unbound" ? '<iframe src="/frame"></iframe>' : ""));
    });
    let browser;
    try {
      await new Promise(resolve => server.listen(0, "127.0.0.1", resolve));
      const origin = `http://127.0.0.1:${server.address().port}`;
      browser = await chromium.launch({ executablePath, headless: true });
      const context = await browser.newContext(), page = await context.newPage();
      const cdp = await context.newCDPSession(page), observer = await observeFeedbackDocuments(cdp);
      await page.goto(origin + (kind === "earlier-shell" ? "/first" : kind === "iframe-offset" ? "/unbound" : "/"), { waitUntil: "load" });
      if (kind !== "iframe-offset") {
        await page.waitForFunction(() => globalThis.__extraExecuted);
        assert.equal(await page.locator("#extra").count(), 0, "The extra module really removed its DOM element");
      } else {
        assert.equal(observer.messages.length, 0, "The real iframe binding must not enter top-level telemetry");
      }
      if (["earlier-shell", "iframe-offset"].includes(kind)) await page.goto(origin + "/second", { waitUntil: "load" });
      const final = await page.evaluate(() => ({ id: globalThis.__solidCheckerDocument }));
      assert(final.id, "Final top-level document must really bind");
      assert.deepEqual(await page.locator('script[type="module"]').evaluateAll(scripts => scripts.map(script => script.src)), [origin + "/entry.js"]);
      await assert.rejects(observer.finish(page, [{ url: origin + "/entry.js" }], [], final),
        kind === "iframe-offset" ? /own binding/ : /unselected module entry/);
    } finally {
      if (browser) await browser.close();
      await new Promise(resolve => server.close(resolve));
    }
  }, 15000);
