import assert from "node:assert/strict";
import { test } from "vitest";
import { admitFeedbackEntry, observeFeedbackEntry } from "../scripts/feedback-entry.mjs";
import { mkdtempSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const body = '<html><head><script type="module" src="/@id/app-entry"></script></head><body></body></html>';
async function served({ fallback = false, post = false } = {}) {
  const provenance = await observeFeedbackEntry([Promise.resolve([{
    name: "application-shell",
    load(id) { return id === "\0app-entry" ? "import '/src/App.tsx';" : null; },
    configureServer(server) {
      const install = () => server.middlewares.use(async (_req, res, next) => {
        if (fallback) return next();
        await Promise.resolve();
        res.write(body.slice(0, 10)); res.end(body.slice(10));
      });
      if (post) return install;
      install();
    }
  }])]);
  const stack = [], server = { middlewares: { use(fn) { stack.push(fn); } } };
  provenance.observer.configureServer(server);
  const after = await provenance.plugins[0].configureServer(server); if (after) await after();
  stack.push((_req, res) => res.end(body));
  const req = { url: "/" }, res = { statusCode: 200, getHeader: () => "text/html", write() {}, end() {} };
  let index = 0;
  const next = () => stack[index++](req, res, next);
  await next();
  await provenance.plugins[0].load("\0app-entry");
  return provenance;
}
const response = text => ({ url: () => "http://127.0.0.1:1234/", status: () => 200,
  headers: () => ({ "content-type": "text/html" }), body: async () => Buffer.from(text) });
const options = provenance => ({ response: response(body), scripts: ["/@vite/client", "/@id/virtual:solid-checker-feedback-entry", "/@id/app-entry"],
  origin: "http://127.0.0.1:1234", appTree: "/application", provenance,
  server: { transformRequest: async url => { assert.equal(url, "app-entry"); return { code: "entry" }; },
    moduleGraph: { getModuleByUrl: async url => { assert.equal(url, "app-entry"); return { id: "\0app-entry" }; } } },
  record() { assert.fail("virtual entry is pinned by its loader output"); } });
for (const post of [false, true]) test(`generated HTML needs the config middleware and exact entry loader output (post=${post})`, async () => {
  const provenance = await served({ post });
  const result = await admitFeedbackEntry(options(provenance));
  assert.equal(result.kind, "application-served-html"); assert.equal(result.html.generator.plugin, "application-shell");
  assert.match(result.html.sha256, /^sha256:/); assert.equal(result.entries[0].id, "\0app-entry");
  assert.match(result.entries[0].sha256, /^sha256:/);
  assert(result.generators.some(row => row.hook === "middleware"));
});
test("Vite fallback HTML after an app middleware calls next is not generated-entry proof", async () => {
  const provenance = await served({ fallback: true });
  assert.deepEqual(provenance.html, []);
  await assert.rejects(admitFeedbackEntry(options(provenance)), /not served by an application/);
});
test("a changed HTML body refuses provenance admission", async () => {
  const provenance = await served();
  await assert.rejects(admitFeedbackEntry({ ...options(provenance), response: response(body + "changed") }), /not served by an application/);
});
for (const scripts of [[], ["/@vite/client"], ["/@id/virtual:solid-checker-feedback-entry"], ["https://foreign.test/entry.js"]]) {
  test(`missing app entry refuses even with generated HTML: ${JSON.stringify(scripts)}`, async () => {
    await assert.rejects(admitFeedbackEntry({ ...options(await served()), scripts }), /no proven application module entry/);
  });
}
test("unresolved entry or virtual output with no app loader refuses", async () => {
  const provenance = await served();
  const input = options(provenance);
  await assert.rejects(admitFeedbackEntry({ ...input, server: { transformRequest: async () => null } }), /no proven application module entry/);
  provenance.modules.clear();
  await assert.rejects(admitFeedbackEntry(input), /no proven application module entry/);
});
test("Vite wrapped virtual URLs are decoded before invoking the transform and module graph APIs", async () => {
  const provenance = await served(), calls = [];
  const input = options(provenance);
  const result = await admitFeedbackEntry({ ...input, scripts: ["/@id/__x00__app-entry?client=true"], server: {
    transformRequest: async url => { calls.push(url); return { code: "entry" }; },
    moduleGraph: { getModuleByUrl: async url => { calls.push(url); return { id: "\0app-entry" }; } }
  } });
  assert.deepEqual(calls, ["\0app-entry?client=true", "\0app-entry?client=true"]);
  assert.equal(result.entries[0].url, "/@id/__x00__app-entry?client=true");
});
test("generated HTML can name a pinned physical app entry, while a file outside that tree remains refused", async () => {
  const root = realpathSync(mkdtempSync(join(tmpdir(), "solid-feedback-entry-file-")));
  try {
    const path = join(root, "entry.tsx"); writeFileSync(path, "export const app = 1;");
    const pins = [], input = options(await served()); input.provenance.modules.clear();
    const server = { transformRequest: async () => ({ code: "entry" }), moduleGraph: { getModuleByUrl: async () => ({ id: path, file: path }) } };
    const result = await admitFeedbackEntry({ ...input, appTree: root, server, record: path => pins.push(path) });
    assert.deepEqual(pins, [path]); assert.equal(result.entries[0].path, path);
    await assert.rejects(admitFeedbackEntry({ ...input, appTree: join(root, "other-app"), server }), /no proven application module entry/);
  } finally { rmSync(root, { recursive: true, force: true }); }
});
test("missing, unsuccessful or non-HTML application entry responses refuse", async () => {
  const input = options(await served());
  for (const entry of [null, { ...input.response, status: () => 404 }, { ...input.response, headers: () => ({ "content-type": "application/json" }) }]) {
    await assert.rejects(admitFeedbackEntry({ ...input, response: entry }), /did not return success|did not serve HTML/);
  }
});
