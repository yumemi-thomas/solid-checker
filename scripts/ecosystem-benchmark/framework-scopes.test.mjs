import { test } from "vitest";
import assert from "node:assert/strict";

import {
  auditFrameworkScope,
  exportMapSha256,
  frameworkScopePolicy
} from "./lib/framework-scopes.mjs";
import { Registry } from "./lib/registry.mjs";

test("the framework-scope registry records selected and complement-excluded entrypoints", () => {
  const policy = frameworkScopePolicy("@tanstack/devtools-a11y", "0.2.2");
  assert.deepEqual(policy.selectedEntrypoints, [
    "./core",
    "./core/production",
    "./solid",
    "./solid/production"
  ]);
  assert.deepEqual(policy.excludedEntrypoints, {
    definition: "all-runtime-exports-except-selected",
    reason: "foreign-framework-adapter"
  });
});
test("export-map hashing is deterministic across object key order", () => {
  const left = { "./solid": { import: "./solid.js", types: "./solid.d.ts" }, "./react": "./react.js" };
  const right = { "./react": "./react.js", "./solid": { types: "./solid.d.ts", import: "./solid.js" } };
  assert.equal(exportMapSha256(left), exportMapSha256(right));
});

test("a changed exact export map is refused before exclusions can drift", () => {
  assert.throws(
    () =>
      auditFrameworkScope("@tanstack/devtools-utils", "0.7.0", {
        name: "@tanstack/devtools-utils",
        version: "0.7.0",
        exports: { "./solid": "./solid.js", "./react": "./react.js" }
      }),
    /framework scope export map drift/
  );
});

test("Registry fetches and memoizes an exact version manifest separately from abbreviated packuments", async () => {
  const calls = [];
  const fetchImpl = async (url, options) => {
    calls.push({ url, accept: options.headers.accept ?? null });
    return new Response(JSON.stringify({ name: "@scope/pkg", version: "1.2.3", exports: { ".": "./index.js" } }));
  };
  const registry = new Registry({ registry: "https://registry.example.test", fetchImpl });
  const first = await registry.versionManifest("@scope/pkg", "1.2.3");
  const second = await registry.versionManifest("@scope/pkg", "1.2.3");
  assert.equal(first, second);
  assert.deepEqual(calls, [
    { url: "https://registry.example.test/@scope%2Fpkg/1.2.3", accept: null }
  ]);
});
