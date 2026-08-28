import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "vitest";

import { loadDialectManifests } from "./dialect-manifests.mjs";

/** Loads one synthetic dialect tree through the real validator. */
function load(contracts) {
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
