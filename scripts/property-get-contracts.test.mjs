import assert from "node:assert/strict";
import { describe, test } from "vitest";
import { propertyGetProbeDigest, validatePropertyGets } from "./lib/property-get-contracts.mjs";

const call = {
  callbacks: [{ from: { arg: 0 }, operation: "copy" }],
  operations: [{ id: "copy", kind: "invoke", protocol: "get-enumerable-string-values",
    trigger: { event: "call" }, at: { event: "call", schedule: "same-stack" },
    tracking: "ambient-at-execution", owner: { source: "ambient-at-execution" },
    count: { scope: "call", min: 1, max: 1 } }]
};
describe("authored property-Get gate", () => {
  test("refuses a broad Get disguised as guaranteed property execution", () => {
    validatePropertyGets(call);
    for (const field of ["tracking", "count", "owner"]) {
      const changed = structuredClone(call);
      changed.operations[0][field] = field === "tracking" ? "untracked" : {};
      assert.throws(() => validatePropertyGets(changed));
    }
    assert.throws(() => validatePropertyGets({ ...call, closed: ["callbacks"] }));
    assert.throws(() => validatePropertyGets({ ...call, cases: [] }));
    const member = structuredClone(call);
    member.callbacks[0].from.path = ["count"];
    assert.throws(() => validatePropertyGets(member));
  });
  test("every changed claim, program, runtime or artifact invalidates a probe", () => {
    const spec = { package: "p", version: "1", solidRuntime: [{ version: "rc.13", integrity: "a" }], exports: { V: { call } } };
    const digest = (s = spec, misuse = "n()", correct = "untrack(n)", cases = [{ snapshotRoot: "a" }]) =>
      propertyGetProbeDigest(s, "V", { file: "V", rule: "strict-read-untracked" }, misuse, correct, cases);
    const original = digest();
    assert.notEqual(original, digest(spec, "other()"));
    assert.notEqual(original, digest(spec, "n()", "other()"));
    assert.notEqual(original, digest(spec, "n()", "untrack(n)", [{ snapshotRoot: "b" }]));
    assert.notEqual(original, digest({ ...spec, solidRuntime: [{ version: "rc.13", integrity: "b" }] }));
    assert.equal(propertyGetProbeDigest({ ...spec, exports: { V: { call: {} } } }, "V", {}, "", "", []), undefined);
  });
});
