import assert from "node:assert/strict";
import test from "node:test";

import { exportSubpaths } from "./lib/contract-probe-harness.mjs";

test("a subpath map is returned unchanged", () => {
  const exports = { ".": "./index.js", "./web": "./web/index.js" };
  assert.deepEqual(exportSubpaths(exports), exports);
});

test("a conditions-only map describes the root entrypoint", () => {
  // @solid-primitives/scheduled ships exactly this shape. Reading its keys as
  // entrypoints invents an entrypoint named "import" and loses ".", so the
  // contract's "." looks stale and the runtime's "import" looks missing.
  const exports = { import: { types: "./dist/index.d.ts", default: "./dist/index.js" } };
  assert.deepEqual(exportSubpaths(exports), { ".": exports });
});

test("string sugar describes the root entrypoint", () => {
  assert.deepEqual(exportSubpaths("./index.js"), { ".": "./index.js" });
});

test("a map mixing subpaths keeps its subpath reading", () => {
  const exports = { ".": { default: "./index.js" }, "./store": "./store.js" };
  assert.deepEqual(exportSubpaths(exports), exports);
});

test("an absent or empty exports field yields no entrypoints", () => {
  assert.deepEqual(exportSubpaths(undefined), {});
  assert.deepEqual(exportSubpaths({}), {});
});
