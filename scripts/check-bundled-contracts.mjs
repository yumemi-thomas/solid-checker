#!/usr/bin/env bun

// Checks active stable-v1 bundle indexes without reimplementing semantic
// expansion in JavaScript. The Phase 19 cut intentionally has zero active
// cases until policy-2 certification can reconstruct one.

import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, readFileSync, readdirSync, realpathSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { root } from "./dialect-manifests.mjs";
import { sourceDigest as typefactsSourceDigest } from "./typefacts-source-identity.mjs";
import { sourceDigest as probeHarnessSourceDigest } from "./probe-harness-source-identity.mjs";

function fail(message) {
  throw new Error(`bundled contracts: ${message}`);
}

/// The Makefile's `CERTIFICATION_ENV`, recomputed here.
///
/// `cargo run` below builds into `rust/target/debug` — the very tree
/// `make build-checker-debug` populates. Running it without these variables
/// therefore rebuilds that binary *without* the compiled-in pins, because
/// `option_env!` is part of the crate fingerprint: the next gate to use
/// `rust/target/debug/solid-checker-rust` silently loses Type Facts
/// certification and probe authority, and every probe-gate assertion in the
/// test suite quietly returns early. `make contract-conformance` and
/// `scripts/verify.sh` both reach this script, and neither exported them.
///
/// An already-exported value wins, so an explicit pin from a caller is never
/// overridden.
function certificationEnvironment() {
  const digestOf = path => `sha256:${createHash("sha256").update(readFileSync(path)).digest("hex")}`;
  const computed = {
    SOLID_TYPEFACTS_CERTIFICATION_SHA256: () => digestOf(join(root, "bin/solid-typefacts")),
    SOLID_TYPEFACTS_SOURCE_MANIFEST_SHA256: () => `sha256:${typefactsSourceDigest(root)}`,
    SOLID_CHECKER_PROBE_HARNESS_SHA256: () => `sha256:${probeHarnessSourceDigest(root)}`,
    SOLID_CHECKER_PROBE_NODE_SHA256: () => digestOf(probeNodeRealPath())
  };
  const environment = { ...process.env };
  for (const [name, compute] of Object.entries(computed)) {
    if (environment[name]) continue;
    try {
      environment[name] = compute();
    } catch (error) {
      fail(
        `cannot compute ${name} for the bundle checker's build (${error.message}); a build without it would drop the pins from rust/target/debug`
      );
    }
  }
  return environment;
}

/// The Node executable the Makefile's `PROBE_NODE` resolves, by real path.
///
/// Deliberately not `process.execPath`: this script runs under Bun, and
/// pinning Bun's bytes would compile a digest into the verifier that no probe
/// launch can ever match — and would give this build a different fingerprint
/// from `make build-checker-debug`'s, rebuilding the same tree back and forth.
function probeNodeRealPath() {
  const explicit = process.env.PROBE_NODE || process.env.SOLID_CHECKER_PROBE_NODE;
  if (explicit) return realpathSync(explicit);
  for (const directory of (process.env.PATH ?? "").split(":")) {
    if (!directory) continue;
    try {
      return realpathSync(join(directory, "node"));
    } catch {
      continue;
    }
  }
  throw new Error("no node executable on PATH");
}

/// One bundle case's version contract, checked apart from the filesystem walk
/// so both arms of it are reachable from a test.
///
/// The two numbers are *independent version namespaces*: the main document is
/// the first stable public `schemaVersion: 1`, while Phase 18 cut acceptance
/// receipts to `receiptVersion: 2`. This function used to demand receipt
/// version 1, which would have rejected every receipt Phase 18 onwards issues.
/// Nobody noticed because both bundle indexes are empty, so the branch never
/// ran on real data — which is exactly why it has a test now.
export function validateBundleCase({ documentPath, receiptPath, document, receipt }) {
  if (document.schemaVersion !== 1 || document.format !== "solid-reactivity-contract") {
    fail(`${documentPath} is not a stable-v1 main document`);
  }
  if (receipt.receiptVersion !== 2 || typeof receipt.wireDigest !== "string") {
    fail(`${receiptPath} is not a proof-issued acceptance receipt`);
  }
}

/// Walks both physical bundle locations and returns how many cases were
/// checked. Never launches the Rust checker; `main` does that first.
export function checkBundleIndexes() {
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
        validateBundleCase({
          documentPath,
          receiptPath,
          document: JSON.parse(readFileSync(documentPath, "utf8")),
          receipt: JSON.parse(readFileSync(receiptPath, "utf8"))
        });
        contracts += 1;
      }
    }
  }
  return contracts;
}

function main() {
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
    { cwd: root, stdio: "inherit", env: certificationEnvironment() }
  );
  if (generated.error) fail(`cannot launch the Rust bundle checker: ${generated.error.message}`);
  if (generated.status !== 0) process.exit(generated.status ?? 1);
  const contracts = checkBundleIndexes();
  console.log(
    `checked ${contracts} active policy-2 bundle documents across both physical locations`
  );
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main();
