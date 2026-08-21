import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { createRequire } from "node:module";
import { readFileSync } from "node:fs";
import test from "node:test";

const require = createRequire(import.meta.url);
const { checkSync } = require("../node.cjs");

test("checks an in-memory project through WASI", () => {
  const projectId = "/workspace/example/tsconfig.json";
  const path = "/workspace/example/src/App.tsx";
  const source = "export default function App() { return <main />; }\n";
  const typeFacts = {
    schema: 2,
    generation: 1,
    projectId,
    sources: [{
      path,
      sha256: `sha256:${createHash("sha256").update(source).digest("hex")}`
    }],
    entities: [],
    symbols: [],
    files: []
  };
  const snapshot = JSON.parse(checkSync(JSON.stringify({
    projectId,
    generation: 1,
    sources: [{
      path,
      source,
      compilerOptions: {
        moduleName: "dom",
        generate: "dom",
        hydratable: false,
        dev: false,
        effectWrapper: "",
        wrapConditionals: true,
        staticMarker: "_$",
        builtIns: []
      }
    }],
    typeFacts
  })));

  assert.equal(snapshot.status, "certified");
  assert.deepEqual(snapshot.findings, []);
});

test("documents the intentionally absent preference override channel", () => {
  const readme = readFileSync(new URL("../README.md", import.meta.url), "utf8");
  const declarations = readFileSync(new URL("../index.d.ts", import.meta.url), "utf8");
  assert.match(readme, /cannot read `.solid-checker\/rule-options\.json`/);
  assert.match(readme, /all `prefer-\*` rules run/);
  assert.doesNotMatch(declarations, /presets|enableRules/);
});
