import assert from "node:assert/strict";
import { test } from "vitest";
import { packageInvestigation } from "./package.mjs";

const manifest = { rows: [{ package: "@example/thing", probes: [{ id: "one" }] }] };

test("a package investigation builds the binary it measures and preserves diagnostic artifacts", () => {
  for (const profile of ["debug", "release"]) {
    const plan = packageInvestigation({ PACKAGE: "@example/thing", ECOSYSTEM_PROFILE: profile }, manifest);
    assert.equal(plan.target, `build-checker-${profile}`);
    assert.ok(plan.binary.endsWith(`/${profile}/solid-checker-rust`));
    assert.deepEqual(plan.args.slice(1, 3), ["--package", "@example/thing"]);
    for (const flag of ["--include-graph", "--keep-temp", "--attempt-certification", "--probe-recipe-corpus"]) {
      assert.ok(plan.args.includes(flag));
    }
  }
});

test("package typos, empty selections and unsupported profiles fail before building", () => {
  for (const PACKAGE of [undefined, "thing", "@example/missing"]) {
    assert.throws(() => packageInvestigation({ PACKAGE }, manifest), /exact official package/);
  }
  assert.throws(() => packageInvestigation({ PACKAGE: "@example/thing", ECOSYSTEM_PROFILE: "verify" }, manifest), /must be debug or release/);
});
