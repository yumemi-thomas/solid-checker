import { test } from "node:test";
import assert from "node:assert/strict";
import { validateGetterMembers, lazyGetterProbeDigest } from "./lib/lazy-getter-contracts.mjs";

const claim = () => ({
  call: { operations: [{ id: "return", kind: "return", output: {
    kind: "object", closed: ["properties"], properties: [
      { name: "path", value: { kind: "getter", reactive: true } },
      { name: "key", value: { kind: "getter", reactive: false } },
      { name: "query", value: { kind: "store" } }
    ]
  } }] }, getterClosures: { return: "dist/routing.js:490-528" }
});

test("getter members require exact enumeration, Get flag and their own citation", () => {
  validateGetterMembers("fixture", claim());
  for (const mutate of [
    c => delete c.getterClosures,
    c => delete c.call.operations[0].output.closed,
    c => delete c.call.operations[0].output.properties[0].value.reactive,
    c => c.call.operations[0].output.properties.push(c.call.operations[0].output.properties[0]),
    c => c.call.operations[0].output.properties[0].value.value = { kind: "plain" },
    c => c.getterClosures.missing = "source.js:1",
    c => c.call.operations[0].output = { kind: "getter", reactive: true },
    c => c.call.operations[0].output = { kind: "tuple", items: [c.call.operations[0].output] }
  ]) {
    const c = claim(); mutate(c);
    assert.throws(() => validateGetterMembers("fixture", c));
  }
});

test("getter semantics and both programs are bound into fresh probe identity", () => {
  const spec = { package: "fixture", version: "1.0.0", exports: { make: claim() } };
  const digest = misuse => lazyGetterProbeDigest(spec, "make", {}, misuse, "correct", []);
  const original = digest("misuse");
  assert.notEqual(original, digest("different misuse"));
  spec.exports.make.call.operations[0].output.properties[0].value.reactive = false;
  assert.notEqual(original, digest("misuse"));
});

test("prototype getter dispatch is a separate vocabulary", () => {
  validateGetterMembers("prototype", { call: { operations: [{ kind: "return", output: {
    kind: "prototype-instance", members: [{ name: "size", kind: "getter", tracks: [] }]
  } }] } });
});
