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
  if (!recipes(claim.call).length) return undefined;
  validateLazyGetters(`${spec.package}#${name}`, claim);
  return createHash("sha256").update(JSON.stringify({
    format: "solid-checker:authored-lazy-getters-probe:v1",
    package: spec.package, version: spec.version, runtime: spec.solidRuntime,
    cases, claim, pair, misuse, correct
  })).digest("hex");
}
