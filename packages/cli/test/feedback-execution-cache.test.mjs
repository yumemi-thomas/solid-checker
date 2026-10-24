import assert from "node:assert/strict";
import { test } from "vitest";
import { createHash } from "node:crypto";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { EXECUTION_CACHE_FORMAT, createExecutionCache, executionCacheKey, harnessDigest, stalePins, toolingIdentity }
  from "../scripts/feedback-execution-cache.mjs";

const sha256 = text => `sha256:${createHash("sha256").update(text).digest("hex")}`;
const temporary = fn => {
  const directory = mkdtempSync(join(tmpdir(), "feedback-execution-cache-"));
  try { return fn(directory); } finally { rmSync(directory, { recursive: true, force: true }); }
};

test("a stored browser half is replayed only while every runtime input still matches", () => temporary(directory => {
  const input = join(directory, "App.tsx");
  writeFileSync(input, "export const App = () => null;\n");
  const cache = createExecutionCache(join(directory, "cache"));
  const body = { runtimeInputs: [{ path: input, sha256: sha256("export const App = () => null;\n") }], diagnostics: [{ code: "X" }] };
  const key = executionCacheKey({ feedbackFacts: [] });
  assert.equal(cache.read(key), null, "an absent entry is a miss");
  cache.write(key, body);
  assert.deepEqual(cache.read(key), body);
  // A file Vite served changed: the recorded run no longer describes it.
  writeFileSync(input, "export const App = () => 1;\n");
  assert.equal(cache.read(key), null);
  assert.equal(stalePins(body.runtimeInputs).length, 1);
  // A file it served is gone.
  rmSync(input);
  assert.equal(cache.read(key), null);
}));

test("every input class of the browser half moves the key", () => {
  const base = { harness: "h", tools: [{ name: "vite", sha256: "v" }], browser: "b", project: "/p/tsconfig.json",
    manifest: { project: "/p/tsconfig.json" }, sources: [["/p/App.tsx", "s"]], feedbackFacts: [{ path: "/p/App.tsx", functions: [] }],
    typingErrors: 0, scenario: { steps: [] }, responses: [], runtimeOptions: {} };
  const key = executionCacheKey(base);
  assert.equal(executionCacheKey({ ...base }), key, "the key is a pure function of its parts");
  for (const [name, value] of Object.entries({ harness: "h2", tools: [{ name: "vite", sha256: "v2" }], browser: "b2",
    project: "/q/tsconfig.json", manifest: { project: "/q" }, sources: [["/p/App.tsx", "s2"]],
    feedbackFacts: [{ path: "/p/App.tsx", functions: [{ name: "App" }] }], typingErrors: 1, scenario: { steps: [{ click: "#x" }] },
    responses: [{ url: "/api" }], runtimeOptions: { root: "/p" } })) {
    assert.notEqual(executionCacheKey({ ...base, [name]: value }), key, `${name} must be part of the key`);
  }
});

test("an entry of another format, or filed under another key, is a miss", () => temporary(directory => {
  const cache = createExecutionCache(directory);
  const key = executionCacheKey({ a: 1 }), other = executionCacheKey({ a: 2 });
  cache.write(key, { runtimeInputs: [] });
  const file = join(directory, `${key.replace(/^sha256:/, "")}.json`);
  writeFileSync(file, JSON.stringify({ format: EXECUTION_CACHE_FORMAT + 1, key, body: { runtimeInputs: [] } }));
  assert.equal(cache.read(key), null);
  writeFileSync(file, JSON.stringify({ format: EXECUTION_CACHE_FORMAT, key: other, body: { runtimeInputs: [] } }));
  assert.equal(cache.read(key), null);
  writeFileSync(file, "{ not json");
  assert.equal(cache.read(key), null);
}));

test("the harness digest and tooling identity are content digests", () => temporary(directory => {
  writeFileSync(join(directory, "a.mjs"), "export const a = 1;\n");
  const before = harnessDigest(directory);
  writeFileSync(join(directory, "a.mjs"), "export const a = 2;\n");
  assert.notEqual(harnessDigest(directory), before);
  writeFileSync(join(directory, "package.json"), "{}");
  // A package the tooling root cannot resolve is named, not dropped.
  assert.deepEqual(toolingIdentity(directory, ["not-a-real-package-name"]), [{ name: "not-a-real-package-name", missing: true }]);
}));
