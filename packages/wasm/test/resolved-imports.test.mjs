import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

const require = createRequire(import.meta.url);
const { checkSync } = require("../node.cjs");
const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");
const FIXTURE = join(ROOT, "fixtures/reactive-ir/package-callback-consumer");
const DOCUMENT = readFileSync(
  join(FIXTURE, "node_modules/reactive-package/solid-reactivity.json"),
  "utf8"
);
const RECEIPT = readFileSync(
  join(FIXTURE, "node_modules/reactive-package/solid-reactivity.receipt.json"),
  "utf8"
);
const RECEIPT_VALUE = JSON.parse(RECEIPT);
const CATALOG = JSON.parse(
  readFileSync(join(FIXTURE, ".solid-checker/accepted-contracts.json"), "utf8")
);
const PACKAGE_MANIFEST = readFileSync(
  join(FIXTURE, "node_modules/reactive-package/package.json"),
  "utf8"
);
const SOURCE = "export const answer = 42;\n";

function rebaseImport(root) {
  const input = structuredClone(CATALOG.contracts[0].import);
  const packageRoot = join(root, "node_modules/reactive-package");
  const manifest = join(packageRoot, "package.json");
  input.importer = join(root, "App.ts");
  input.packageRoot = packageRoot;
  input.packageManifest.path = manifest;
  input.runtime.path = manifest;
  input.declarations.path = manifest;
  for (const binding of Object.values(input.exports)) {
    binding.runtime.module.path = manifest;
    binding.declarations.module.path = manifest;
  }
  return input;
}

function request(root, acceptedContracts) {
  const path = join(root, "App.ts");
  return {
    projectId: join(root, "tsconfig.json"),
    generation: 1,
    sources: [{ path, source: SOURCE }],
    typeFacts: {
      schema: 3,
      generation: 1,
      projectId: join(root, "tsconfig.json"),
      sources: [
        { path, sha256: `sha256:${createHash("sha256").update(SOURCE).digest("hex")}` }
      ],
      entities: [],
      symbols: [],
      files: []
    },
    ...(acceptedContracts === undefined ? {} : { acceptedContracts })
  };
}

function project() {
  const root = mkdtempSync(join(tmpdir(), "solid-checker-wasm-v2-"));
  const packageRoot = join(root, "node_modules/reactive-package");
  mkdirSync(packageRoot, { recursive: true });
  writeFileSync(join(packageRoot, "package.json"), PACKAGE_MANIFEST);
  return root;
}

function check(value) {
  return JSON.parse(checkSync(JSON.stringify(value)));
}

test("omitting acceptedContracts leaves external behavior unaccepted", () => {
  const root = project();
  try {
    assert.equal(check(request(root)).status, "certified");
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("a receipt-issued temporary-v2 document crosses the WASM boundary", () => {
  const root = project();
  try {
    const snapshot = check(
      request(root, [{ document: DOCUMENT, receipt: RECEIPT, import: rebaseImport(root) }])
    );
    assert.equal(snapshot.status, "certified");
    assert.deepEqual(snapshot.packageSummaries, [
      {
        name: "reactive-package",
        version: "1.0.0",
        contractHash: RECEIPT_VALUE.semanticDigest,
        evidence: "accepted",
        exportsAnalyzed: 0
      }
    ]);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("a receipt mismatch is refused before analysis", () => {
  const root = project();
  try {
    const receipt = JSON.stringify({
      ...RECEIPT_VALUE,
      semanticDigest: `sha256:${"0".repeat(64)}`
    });
    assert.throws(
      () => check(request(root, [{ document: DOCUMENT, receipt, import: rebaseImport(root) }])),
      /semanticDigest|semantic digest/i
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("legacy schema-version-1 documents have no compatibility decoder", () => {
  const root = project();
  try {
    const document = JSON.stringify({ ...JSON.parse(DOCUMENT), schemaVersion: 1 });
    assert.throws(
      () => check(request(root, [{ document, receipt: RECEIPT, import: rebaseImport(root) }])),
      /schema version 1.*expected 2/i
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
