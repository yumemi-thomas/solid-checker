import assert from "node:assert/strict";
import { createRequire } from "node:module";
import test from "node:test";

const require = createRequire(import.meta.url);
const { checkSync } = require("../node.cjs");

test("checks an in-memory project through WASI", () => {
  const projectId = "/workspace/example/tsconfig.json";
  const typeFacts = {
    schema: 2,
    generation: 1,
    projectId,
    sources: [],
    entities: [],
    symbols: [],
    files: []
  };
  const snapshot = JSON.parse(checkSync(JSON.stringify({
    projectId,
    generation: 1,
    sources: [],
    typeFacts
  })));

  assert.equal(snapshot.status, "certified");
  assert.deepEqual(snapshot.findings, []);
});
