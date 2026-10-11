import assert from "node:assert/strict";
import { createHash } from "node:crypto";

function recipes(call) {
  return (call?.operations ?? []).flatMap(operation => {
    const output = operation.output;
    const values = output?.kind === "tuple" ? output.items ?? [] : [output];
    return values.filter(value => value?.kind === "lazy-getter-object").map(value => [operation, value]);
  });
}

export function validateLazyGetters(where, claim) {
  validateGetterMembers(where, claim);
  const rows = recipes(claim.call);
  const ids = new Set(rows.map(([operation]) => operation.id));
  const cited = claim.lazyGetterClosures ?? {};
  for (const id of Object.keys(cited)) assert(ids.has(id), `${where}: lazyGetterClosures names no recipe ${id}`);
  for (const [operation, recipe] of rows) {
    assert.equal(operation.kind, "return", `${where}: lazy getters require a return`);
    assert(Array.isArray(recipe.keys), `${where}: lazy getters require key provenance`);
    assert((recipe.keys.length > 0) !== Number.isInteger(recipe.from), `${where}: exact keys or argument index, never both`);
    assert(typeof cited[operation.id] === "string" && /\S+:\d+/.test(cited[operation.id]),
      `${where}: lazy getter recipe needs installed source citation`);
  }
}

export function lazyGetterProbeDigest(spec, name, pair, misuse, correct, cases) {
  const claim = spec.exports[name];
  if (!recipes(claim.call).length && !getterObjects(claim.call).length) return undefined;
  validateLazyGetters(`${spec.package}#${name}`, claim);
  return createHash("sha256").update(JSON.stringify({
    format: "solid-checker:authored-lazy-getters-probe:v1",
    package: spec.package, version: spec.version, runtime: spec.solidRuntime,
    cases, claim, pair, misuse, correct
  })).digest("hex");
}

function getterObjects(call) {
  return (call?.operations ?? []).filter(operation => operation.output?.kind === "object"
    && operation.output.properties?.some(property => property.value?.kind === "getter"));
}

// ADR 0272. Existing object members describe values; this leaf describes Get.
export function validateGetterMembers(where, claim) {
  const rows = getterObjects(claim.call);
  const leaves = value => !value || typeof value !== "object" || value.kind === "prototype-instance" ? []
    : [...(value.kind === "getter" ? [value] : []), ...Object.values(value).flatMap(leaves)];
  const direct = new Set(rows.flatMap(row => row.output.properties.map(property => property.value)));
  assert(leaves(claim.call).every(leaf => direct.has(leaf)), `${where}: getters require direct members of a whole returned object`);
  const citations = claim.getterClosures ?? {};
  for (const id of Object.keys(citations)) assert(rows.some(row => row.id === id), `${where}: getterClosures names no getter object ${id}`);
  for (const operation of rows) {
    assert.equal(operation.kind, "return", `${where}: getter members require a return`);
    assert(operation.output.closed?.includes("properties"), `${where}: getter object requires exact members`);
    assert(typeof citations[operation.id] === "string" && /\S+:\d+/.test(citations[operation.id]), `${where}: getter members require installed source citation`);
    const names = new Set();
    for (const property of operation.output.properties) {
      assert(typeof property.name === "string" && property.name.length && !names.has(property.name), `${where}: duplicate/empty getter object member`);
      names.add(property.name);
      assert(["getter", "plain", "store", "reactive"].includes(property.value?.kind), `${where}: unsupported getter object data shape`);
      if (property.value.kind === "getter") {
        assert.equal(typeof property.value.reactive, "boolean", `${where}: getter needs exact reactive flag`);
        assert.deepEqual(Object.keys(property.value).sort(), ["kind", "reactive"], `${where}: getter yields plain data only`);
      }
    }
  }
}
