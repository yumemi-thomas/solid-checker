// Authoring/gate checks for the deliberately narrow value-Get vocabulary.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";

const protocols = new Set(["get-enumerable-string-values", "get-own-enumerable-values"]);
export const propertyGetOperations = call => (call.operations ?? []).filter(operation => protocols.has(operation.protocol));

export function validatePropertyGets(call) {
  for (const operation of propertyGetOperations(call)) {
    const items = (call.callbacks ?? []).filter(item => item.operation === operation.id);
    assert.equal(items.length, 1, "a value Get needs one source item");
    const source = items[0].from;
    assert(Number.isInteger(source.arg) && source.arg >= 0 && source.arg <= 65535, "a value Get needs an exact argument");
    assert.deepEqual(Object.keys(source).filter(key => key !== "path").sort(), ["arg"]);
    assert.deepEqual(source.path ?? [], [], "the initial Get receiver is a bare argument");
    assert.equal(operation.kind, "invoke");
    assert.deepEqual(operation.trigger, { event: "call" });
    assert.deepEqual(operation.at, { event: "call", schedule: "same-stack" });
    assert.equal(operation.tracking, "ambient-at-execution");
    assert.deepEqual(operation.owner, { source: "ambient-at-execution" });
    assert.deepEqual(operation.count, {
      scope: "call", min: operation.protocol === "get-enumerable-string-values" ? 1 : 0, max: 1
    });
    for (const field of ["guard", "output", "composedFrom"])
      assert.equal(operation[field], undefined, `${field} has no meaning on this Get form`);
    for (const field of ["inputs", "resources"])
      assert.deepEqual(operation[field] ?? [], []);
    assert.equal(call.cases, undefined, "partitioned Gets need call-site instantiation first");
    assert(!(call.closed ?? []).includes("callbacks"), "finite getter pairs never close callbacks");
  }
}

// New vocabulary must never borrow an old broad-Get probe result. Bind the
// full claim, runtime integrities, package case identities and both source
// programs. Old claims have no new key and keep their existing result format.
export function propertyGetProbeDigest(spec, name, pair, misuse, correct, cases) {
  if (!propertyGetOperations(spec.exports[name].call).length) return undefined;
  validatePropertyGets(spec.exports[name].call);
  return createHash("sha256").update(JSON.stringify({
    format: "solid-checker:authored-property-get-probe:v1",
    package: spec.package, version: spec.version, runtime: spec.solidRuntime,
    cases, call: spec.exports[name].call, pair, misuse, correct
  })).digest("hex");
}
