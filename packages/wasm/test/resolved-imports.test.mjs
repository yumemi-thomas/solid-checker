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
const DOCUMENT_VALUE = JSON.parse(DOCUMENT);
const POLICY1_RECEIPT = JSON.stringify({ receiptVersion: 1 });
const UNAUTHENTICATED_POLICY2_RECEIPT = JSON.stringify({ receiptVersion: 2 });
const PACKAGE_MANIFEST = readFileSync(
  join(FIXTURE, "node_modules/reactive-package/package.json"),
  "utf8"
);
const SOURCE = "export const answer = 42;\n";

function rebaseImport(root) {
  const contractPackage = DOCUMENT_VALUE.package;
  const artifact = DOCUMENT_VALUE.entrypoints["."];
  const packageRoot = join(root, "node_modules/reactive-package");
  const manifest = join(packageRoot, "package.json");
  const digest = `sha256:${artifact.artifact.sha256}`;
  const file = { path: manifest, digest };
  return {
    authority: "host",
    closure: {
      digest: `sha256:${artifact.artifact.closureSha256}`,
      entries: [{ ...file, path: "./package.json", role: "manifest" }]
    },
    declarationTrace: { branch: "" },
    declarations: file,
    exports: Object.fromEntries(
      Object.keys(artifact.exports).map(name => [
        name,
        {
          declarations: { exportName: name, module: file },
          runtime: { exportName: name, module: file }
        }
      ])
    ),
    importer: join(root, "App.ts"),
    packageIntegrity: contractPackage.integrity,
    packageManifest: file,
    packageName: contractPackage.name,
    packageRoot,
    packageVersion: contractPackage.version,
    requestedEntrypoint: ".",
    runtime: file,
    runtimeTrace: { branch: "" },
    specifier: contractPackage.name
  };
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
  const root = mkdtempSync(join(tmpdir(), "solid-checker-wasm-"));
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

test("policy-1 receipts are obsolete at the WASM boundary", () => {
  const root = project();
  try {
    assert.throws(
      () =>
        check(
          request(root, [
            { document: DOCUMENT, receipt: POLICY1_RECEIPT, import: rebaseImport(root) }
          ])
        ),
      /unsupported acceptance receipt version 1.*expected 2/i
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("policy-2 receipts without issuer provenance are refused before analysis", () => {
  const root = project();
  try {
    assert.throws(
      () =>
        check(
          request(root, [
            {
              document: DOCUMENT,
              receipt: UNAUTHENTICATED_POLICY2_RECEIPT,
              import: rebaseImport(root)
            }
          ])
        ),
      /requires authenticated issuer provenance/i
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("temporary schema-version-2 documents have no compatibility decoder", () => {
  const root = project();
  try {
    const document = JSON.stringify({ ...JSON.parse(DOCUMENT), schemaVersion: 2 });
    assert.throws(
      () =>
        check(
          request(root, [
            { document, receipt: UNAUTHENTICATED_POLICY2_RECEIPT, import: rebaseImport(root) }
          ])
        ),
      /schema version 2.*expected 1/i
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("legacy schema-version-1 documents have no compatibility decoder", () => {
  const root = project();
  try {
    const legacy = JSON.parse(DOCUMENT);
    delete legacy.format;
    const document = JSON.stringify(legacy);
    assert.throws(
      () =>
        check(
          request(root, [
            { document, receipt: UNAUTHENTICATED_POLICY2_RECEIPT, import: rebaseImport(root) }
          ])
        ),
      /contract document cannot be decoded.*missing field.*format/i
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
