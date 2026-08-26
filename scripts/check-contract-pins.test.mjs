import assert from "node:assert/strict";
import { test } from "vitest";

import { verifyPin } from "./check-contract-pins.mjs";

const audited = "sha512-audited==";
const contract = (overrides = {}) => ({
  label: "solid-v9/example",
  file: "pkg/contracts/bundled/solid-v9/example.json",
  expectedName: "example",
  document: { package: { name: "example", version: "1.0.0", integrity: audited } },
  ...overrides,
});

const serving = integrity => () => ({ integrity });

test("a pin matching the registry tarball holds", () => {
  assert.equal(verifyPin(contract(), serving(audited)), undefined);
});

test("a version-only pin fails, because it cannot be falsified", () => {
  const document = { package: { name: "example", version: "1.0.0" } };
  const failure = verifyPin(contract({ document }), serving(audited));
  assert.match(failure, /by version alone/);
  assert.match(failure, /bun info example@1\.0\.0 dist\.integrity/);
});

test("a republished release fails even though the version still matches", () => {
  const failure = verifyPin(contract(), serving("sha512-republished=="));
  assert.match(failure, /is sha512-republished== in the registry/);
  assert.match(failure, /audited against sha512-audited==/);
});

test("a contract describing another package fails before any lookup", () => {
  const document = { package: { name: "other", version: "1.0.0", integrity: audited } };
  const failure = verifyPin(contract({ document }), () => {
    throw new Error("the registry must not be consulted for a mismatched name");
  });
  assert.match(failure, /describes "other", not example/);
});

test("a registry lookup failure is reported, not treated as a pass", () => {
  const failure = verifyPin(contract(), () => ({ error: "registry lookup failed: ENOTFOUND" }));
  assert.match(failure, /registry lookup failed: ENOTFOUND/);
});
