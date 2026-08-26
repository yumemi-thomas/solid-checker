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
      schemaVersion: 1,
      id: "solid-v9",
      ruleManifest: "packages/cli/lib/rules-solid-v9.json",
      contracts
    })
  );
  return () => loadDialectManifests({ projectRoot });
}

const generated = {
  package: "solid-js",
  packagePathEnv: "SOLID_V9_SOLID_JS_PACKAGE",
  defaultPackagePath: "node_modules/solid-js",
  generatorTarget: "solid-v9/solid-js",
  reviewContract: "rust/crates/solid-dialect/contracts/solid-v9/solid-js.json",
  exportsIndex: "rust/crates/solid-dialect/src/exports/solid_v9_solid_js.rs",
  bundledContract: "pkg/contracts/bundled/solid-v9/solid-js.json"
};

const overlay = {
  package: "@solid-primitives/scheduled",
  bundledContract: "pkg/contracts/bundled/solid-v9/scheduled.json",
  generated: false
};

test("a hand-authored overlay declares only its package and bundled artifact", () => {
  const manifests = load([generated, overlay])();
  assert.deepEqual(
    manifests[0].contracts.map(contract => contract.package),
    ["solid-js", "@solid-primitives/scheduled"]
  );
});

test("a generated contract still requires every generator field", () => {
  const { generatorTarget, ...withoutTarget } = generated;
  assert.throws(load([withoutTarget]), /requires non-empty contracts\[\]\.generatorTarget/);
});

test("generator fields are refused beside generated: false", () => {
  // A half-filled entry is someone leaving fields out of a generated contract;
  // accepting it silently would let a package skip generation and its gate.
  assert.throws(
    load([{ ...generated, generated: false }]),
    /packagePathEnv is not allowed when generated is false/
  );
});

test("generated must be a boolean when present", () => {
  assert.throws(load([{ ...generated, generated: "false" }]), /must be a boolean/);
});

test("one package cannot be declared twice in a dialect", () => {
  assert.throws(
    load([generated, { ...generated, generatorTarget: "solid-v9/solid-js-again" }]),
    /declares solid-js twice/
  );
});
