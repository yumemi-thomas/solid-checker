import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "vitest";

import { loadDialectManifests } from "./dialect-manifests.mjs";

/**
 * The compiler wiring every manifest must declare, so that these cases can be
 * about the package inventory. `compilerIdentity` has its own cases below.
 */
const compilerIdentity = {
  document: "docs/solid-v9/compiler-identity.json",
  adapter: "rust/dialects/solid-v9/compiler/src/lib.rs",
  conformance: "docs/solid-v9/conformance.json",
  report: "docs/solid-v9/compiler-facts.md",
  cargoPackage: "solidjs-v9-compiler",
  cargoSourcePrefix: "git+https://example.invalid/solid?"
};

/** Loads one synthetic dialect tree through the real validator. */
function load(contracts, identity = compilerIdentity) {
  const projectRoot = mkdtempSync(join(tmpdir(), "solid-checker-manifest-"));
  const dialect = join(projectRoot, "rust", "dialects", "solid-v9");
  mkdirSync(dialect, { recursive: true });
  writeFileSync(
    join(dialect, "dialect.json"),
    JSON.stringify({
      schemaVersion: 2,
      id: "solid-v9",
      ruleManifest: "packages/cli/lib/rules-solid-v9.json",
      bundleIndex: "pkg/contracts/bundled/solid-v9/bundle-index.json",
      reviewBundleIndex: "rust/crates/solid-dialect/contracts/solid-v9/bundle-index.json",
      ...(identity === null ? {} : { compilerIdentity: identity }),
      contracts
    })
  );
  return () => loadDialectManifests({ projectRoot });
}

const generated = { package: "solid-js", probeRuntime: true };

const overlay = { package: "@solid-primitives/scheduled" };

test("the manifest is only a package inventory plus normalized bundle indexes", () => {
  const manifests = load([generated, overlay])();
  assert.deepEqual(
    manifests[0].contracts.map(contract => contract.package),
    ["solid-js", "@solid-primitives/scheduled"]
  );
});

test("legacy per-document generator and bundle fields are refused", () => {
  assert.throws(
    load([{ ...generated, bundledContract: "pkg/contracts/bundled/solid-v9/solid-js.json" }]),
    /bundledContract is not part of the normalized bundle inventory/
  );
});

test("probeRuntime must be a boolean when present", () => {
  assert.throws(load([{ ...generated, probeRuntime: "true" }]), /probeRuntime must be a boolean/);
});

test("probe modes require an enabled runtime probe", () => {
  assert.throws(
    load([{ package: "solid-js", probeModes: ["client"] }]),
    /has no meaning without probeRuntime/
  );
});

test("one package cannot be declared twice in a dialect", () => {
  assert.throws(
    load([generated, generated]),
    /declares solid-js twice/
  );
});

// The gate that checks a dialect's compiler against its identity documents
// reads these paths from here. A dialect that declared nothing would have been
// assembled and never checked, so absence has to fail the manifest.
test("a dialect must declare the compiler its identity gate checks", () => {
  assert.throws(load([generated], null), /requires compilerIdentity/);
  for (const field of Object.keys(compilerIdentity)) {
    const partial = { ...compilerIdentity };
    delete partial[field];
    assert.throws(
      load([generated], partial),
      new RegExp(`requires non-empty compilerIdentity\\.${field}`),
      `compilerIdentity.${field} must be required`
    );
  }
});

test("unknown compiler wiring fields are refused", () => {
  assert.throws(
    load([generated], { ...compilerIdentity, babelPlugin: "solid-v9/babel" }),
    /compilerIdentity.babelPlugin is not part of the compiler wiring/
  );
});
