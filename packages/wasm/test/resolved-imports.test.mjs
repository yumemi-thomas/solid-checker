import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { createRequire } from "node:module";
import { test } from "vitest";

const require = createRequire(import.meta.url);
const { checkSync } = require("../node.cjs");

const ROOT = "/workspace/app";

function request(source, extra = {}) {
  const path = `${ROOT}/App.ts`;
  return {
    projectId: `${ROOT}/tsconfig.json`,
    generation: 1,
    sources: [{ path, source }],
    typeFacts: {
      schema: 3,
      generation: 1,
      projectId: `${ROOT}/tsconfig.json`,
      sources: [
        { path, sha256: `sha256:${createHash("sha256").update(source).digest("hex")}` }
      ],
      entities: [],
      symbols: [],
      files: []
    },
    ...extra
  };
}

function check(value) {
  return JSON.parse(checkSync(JSON.stringify(value)));
}

test("a project importing no package is certified", () => {
  assert.equal(check(request("export const answer = 42;\n")).status, "certified");
});

// P7 of the contract-removal plan: no package contract is consumed, so a host
// still handing one in is refused rather than silently ignored.
test("acceptedContracts is refused", () => {
  assert.throws(
    () =>
      check(
        request("export const answer = 42;\n", {
          acceptedContracts: [{ document: "{}", receipt: "{}", import: {} }]
        })
      ),
    /acceptedContracts is no longer read/
  );
});

// P5: this request supplies no package files, so a package import stays an
// open obligation rather than reading as plain code.
test("a package import stays uncertifiable", () => {
  const source = [
    'import { createSignal } from "solid-js";',
    'import { watch } from "reactive-package";',
    "export function App() {",
    "  const [count] = createSignal(0);",
    "  watch(() => count());",
    "  return null;",
    "}",
    ""
  ].join("\n");
  const snapshot = check(request(source));
  assert.equal(snapshot.status, "uncertifiable");
  assert.ok(
    snapshot.findings.some(finding => finding.id === "SC9005"),
    JSON.stringify(snapshot.findings)
  );
});
