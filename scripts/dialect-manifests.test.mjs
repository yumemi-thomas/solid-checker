import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "vitest";

import { loadDialectManifests } from "./dialect-manifests.mjs";

/**
 * The compiler wiring every manifest must declare. `compilerIdentity` has its
 * own cases below.
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
function load(extra = {}, identity = compilerIdentity) {
  const projectRoot = mkdtempSync(join(tmpdir(), "solid-checker-manifest-"));
  const dialect = join(projectRoot, "rust", "dialects", "solid-v9");
  mkdirSync(dialect, { recursive: true });
  writeFileSync(
    join(dialect, "dialect.json"),
    JSON.stringify({
      schemaVersion: 2,
      id: "solid-v9",
      ruleManifest: "packages/cli/lib/rules-solid-v9.json",
      ...(identity === null ? {} : { compilerIdentity: identity }),
      ...extra
    })
  );
  return () => loadDialectManifests({ projectRoot });
}

test("the manifest names the dialect, its rules and its compiler", () => {
  const manifests = load()();
  assert.equal(manifests[0].id, "solid-v9");
});

test("the package-contract fields are refused", () => {
  for (const field of ["bundleIndex", "reviewBundleIndex", "contracts"]) {
    assert.throws(
      load({ [field]: "x" }),
      new RegExp(`${field} was removed with package contracts`)
    );
  }
});

// The gate that checks a dialect's compiler against its identity documents
// reads these paths from here. A dialect that declared nothing would have been
// assembled and never checked, so absence has to fail the manifest.
test("a dialect must declare the compiler its identity gate checks", () => {
  assert.throws(load({}, null), /requires compilerIdentity/);
  for (const field of Object.keys(compilerIdentity)) {
    const partial = { ...compilerIdentity };
    delete partial[field];
    assert.throws(
      load({}, partial),
      new RegExp(`requires non-empty compilerIdentity\\.${field}`),
      `compilerIdentity.${field} must be required`
    );
  }
});

test("unknown compiler wiring fields are refused", () => {
  assert.throws(
    load({}, { ...compilerIdentity, babelPlugin: "solid-v9/babel" }),
    /compilerIdentity.babelPlugin is not part of the compiler wiring/
  );
});
