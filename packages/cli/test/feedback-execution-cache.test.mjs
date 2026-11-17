import assert from "node:assert/strict";
import { test } from "vitest";
import { createHash } from "node:crypto";
import { mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { EXECUTION_CACHE_FORMAT, createExecutionCache, executionCacheKey, harnessDigest, localBody, portableBody, stalePins, toolingIdentity }
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

// ADR 1029: a browser half is stored with its paths as placeholders, so another checkout of the same case answers from it, and its
// runtime-input pins are re-hashed at that checkout's paths.
test("a browser half recorded in one checkout answers the same case in another, re-pinned at the new paths", () => temporary(directory => {
  const roots = name => ({ "<<sc:project-root>>": join(directory, name, "case"), "<<sc:tooling-root>>": join(directory, name, "install") });
  for (const name of ["a", "b"]) {
    mkdirSync(join(directory, name, "case"), { recursive: true });
    mkdirSync(join(directory, name, "install"), { recursive: true });
    writeFileSync(join(directory, name, "case/App.tsx"), "export const App = () => null;\n");
    writeFileSync(join(directory, name, "install/vite.js"), "vite");
  }
  const cache = createExecutionCache(join(directory, "shared"));
  const key = executionCacheKey({ project: "tsconfig.json", sources: [["App.tsx", "s"]] });
  const a = roots("a");
  const body = { runtimeInputs: [
    { path: join(a["<<sc:project-root>>"], "App.tsx"), sha256: sha256("export const App = () => null;\n") },
    { path: join(a["<<sc:tooling-root>>"], "vite.js"), sha256: sha256("vite") }],
  diagnostics: [{ code: "X", site: { location: { path: join(a["<<sc:project-root>>"], "App.tsx") } } }] };
  cache.write(key, body, a);
  const stored = readdirSync(join(directory, "shared")).map(name => readFileSync(join(directory, "shared", name), "utf8")).join("");
  assert.ok(!stored.includes(join(directory, "a")), "the stored half holds no checkout path");
  const fromB = cache.read(key, roots("b"));
  assert.equal(fromB.diagnostics[0].site.location.path, join(directory, "b/case/App.tsx"));
  assert.deepEqual(fromB.runtimeInputs.map(pin => pin.path), [join(directory, "b/case/App.tsx"), join(directory, "b/install/vite.js")]);
  // A file b serves differs from what a recorded: a miss, as in one checkout.
  writeFileSync(join(directory, "b/install/vite.js"), "another vite");
  assert.equal(cache.read(key, roots("b")), null);
  assert.deepEqual(localBody(portableBody(body, a), a), body);
}));

test("the tooling identity names each package's manifest relative to the tooling root", () => temporary(directory => {
  for (const name of ["one", "two"]) {
    mkdirSync(join(directory, name, "node_modules/vite"), { recursive: true });
    writeFileSync(join(directory, name, "package.json"), "{}");
    writeFileSync(join(directory, name, "node_modules/vite/package.json"), JSON.stringify({ name: "vite", version: "8.0.0", main: "index.js" }));
    writeFileSync(join(directory, name, "node_modules/vite/index.js"), "");
  }
  const one = toolingIdentity(join(directory, "one"), ["vite"]), two = toolingIdentity(join(directory, "two"), ["vite"]);
  assert.deepEqual(one, two);
  assert.equal(one[0].path, join("node_modules", "vite", "package.json"));
}));

test("the key's manifest keeps what was read and resolved, not the probes that found nothing", async () => {
  const { keyManifest } = await import("../scripts/feedback-execution-cache.mjs");
  const read = { kind: "readFile", path: "<<sc:project-root>>/App.tsx", arguments: [], result: "sha256:a" };
  const found = { kind: "fileExists", path: "<<sc:tooling-root>>/node_modules/solid-js/package.json", arguments: [], result: true };
  const a = { project: "p", inputs: [read, found, { kind: "directoryExists", path: "/a/node_modules", arguments: [], result: false }, { kind: "fileExists", path: "/a/x.d.ts", arguments: [], result: false }] };
  const b = { project: "p", inputs: [read, { kind: "directoryExists", path: "/b/c/node_modules", arguments: [], result: true }, found] };
  assert.deepEqual(keyManifest(a), keyManifest(b));
  // A resolution that now reaches another file is a read of it: the key moves.
  assert.notDeepEqual(keyManifest({ ...b, inputs: [...b.inputs, { kind: "readFile", path: "/b/node_modules/x/index.d.ts", arguments: [], result: "sha256:x" }] }), keyManifest(a));
});
