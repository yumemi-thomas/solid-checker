import assert from "node:assert/strict";
import { describe, test } from "vitest";
import { propertyGetProbeDigest, validatePropertyGets } from "./lib/property-get-contracts.mjs";

import { strictReadProbeDigest, strictReadWireCall, validateStrictReads } from "./lib/strict-read-contracts.mjs";

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

describe("authored strict-read clearing", () => {
  const read = { id: "read", kind: "read", tracking: "untracked", strictRead: "cleared" };
  const claim = { call: { operations: [read], reads: ["read"], closed: ["reads"] },
    closures: { reads: "dist/index.js:98; signals/dev-shared.js:5863-5887" } };
  test("requires an explicit read, untracked tracking and its own citation", () => {
    validateStrictReads("export", claim);
    assert.throws(() => validateStrictReads("export", { ...claim, closures: {} }));
    for (const field of ["kind", "tracking", "strictRead"]) {
      const bad = structuredClone(claim);
      bad.call.operations[0][field] = "unknown";
      assert.throws(() => validateStrictReads("export", bad));
    }
    const cited = { call: { operations: [{ ...read, why: "dist/index.js:98" }] } };
    validateStrictReads("export", cited);
    const wire = strictReadWireCall(cited.call);
    assert.equal(wire.operations[0].why, undefined);
    assert.equal(wire.operations[0].strictRead, "cleared");
    assert.equal(cited.call.operations[0].why, "dist/index.js:98");
  });
  test("nested members require their own evidence and affect probe identity", () => {
    const member = { call: { operations: [{ id: "return", kind: "return", output: {
      kind: "object", properties: { start: { kind: "effectful-callable", call: claim.call } }
    } }] }, closures: claim.closures };
    assert.throws(() => validateStrictReads("export", member));
    member.memberClosures = { "return.start": claim.closures };
    validateStrictReads("export", member);
    const spec = { package: "p", version: "1", solidRuntime: [{ version: "rc.13" }], exports: { V: member } };
    const digest = (s = spec, misuse = "n()", correct = "untrack(n)", cases = [{ snapshotRoot: "a" }]) =>
      strictReadProbeDigest(s, "V", { rule: "strict-read-untracked" }, misuse, correct, cases);
    const original = digest();
    assert.notEqual(original, digest(spec, "other()"));
    assert.notEqual(original, digest(spec, "n()", "other()"));
    assert.notEqual(original, digest(spec, "n()", "untrack(n)", [{ snapshotRoot: "b" }]));
    const changed = structuredClone(spec);
    changed.exports.V.call.operations[0].output.properties.start.call.operations[0].count = { min: 0 };
    assert.notEqual(original, digest(changed));
    assert.equal(strictReadProbeDigest({ ...spec, exports: { V: { call: {} } } }, "V", {}, "", "", []), undefined);
  });
});
