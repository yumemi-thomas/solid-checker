#!/usr/bin/env bun

// Checks active stable-v1 bundle indexes without reimplementing semantic
// expansion in JavaScript. The Phase 19 cut intentionally has zero active
// cases until policy-2 certification can reconstruct one.

import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";

import { root } from "./dialect-manifests.mjs";

function fail(message) {
  throw new Error(`bundled contracts: ${message}`);
}

const generated = spawnSync(
  "cargo",
  [
    "+1.97",
    "run",
    "--manifest-path",
    "rust/Cargo.toml",
    "-p",
    "solid-facts-backend",
    "--bin",
    "solid-contract-bundles",
    "--",
    "--root",
    root,
    "--check"
  ],
  { cwd: root, stdio: "inherit" }
);
if (generated.error) fail(`cannot launch the Rust bundle checker: ${generated.error.message}`);
if (generated.status !== 0) process.exit(generated.status ?? 1);

let contracts = 0;
for (const location of ["pkg/contracts/bundled", "rust/crates/solid-dialect/contracts"]) {
  const directory = join(root, location);
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    if (!entry.isDirectory() || !entry.name.startsWith("solid-v")) continue;
    const indexPath = join(directory, entry.name, "bundle-index.json");
    if (!existsSync(indexPath)) fail(`${indexPath} is missing`);
    const index = JSON.parse(readFileSync(indexPath, "utf8"));
    if (
      index.schemaVersion !== 1 ||
      index.format !== "solid-checker-package-contract-bundle-index" ||
      !Array.isArray(index.contracts)
    ) {
      fail(`${indexPath} is not a stable-v1 bundle index`);
    }
    for (const item of index.contracts) {
      const documentPath = join(directory, entry.name, item.document);
      const receiptPath = join(directory, entry.name, item.receipt);
      const document = JSON.parse(readFileSync(documentPath, "utf8"));
      const receipt = JSON.parse(readFileSync(receiptPath, "utf8"));
      if (document.schemaVersion !== 1 || document.format !== "solid-reactivity-contract") {
        fail(`${documentPath} is not a stable-v1 main document`);
      }
      if (receipt.receiptVersion !== 1 || typeof receipt.wireDigest !== "string") {
        fail(`${receiptPath} is not a proof-issued acceptance receipt`);
      }
      contracts += 1;
    }
  }
}
console.log(`checked ${contracts} active policy-2 bundle documents across both physical locations`);
