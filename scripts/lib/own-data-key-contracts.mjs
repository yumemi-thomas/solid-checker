// Draft authoring guard observations bind to the complete audited claim.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";

function atoms(value) {
  if (!value || typeof value !== "object") return [];
  return [
    ...(Object.hasOwn(value, "ownDataKeys") ? [value] : []),
    ...Object.values(value).flatMap(atoms)
  ];
}

export function validateOwnDataKeys(where, claim) {
  const guards = atoms(claim.call);
  if (!guards.length) return;
  const cites = value => typeof value === "string" && /\S+:\d+/.test(value);
  assert(cites(claim.why) || Object.values(claim.closures ?? {}).some(cites),
    `${where}: ownDataKeys needs an installed-source citation`);
  for (const atom of guards) {
    assert(Number.isInteger(atom.arg) && atom.arg >= 0 && atom.arg <= 65535);
    assert(Object.keys(atom).every(key => ["arg", "path", "ownDataKeys"].includes(key)));
    assert(Array.isArray(atom.path ?? []) && (atom.path ?? []).every(key => typeof key === "string"));
    assert(Array.isArray(atom.ownDataKeys) && atom.ownDataKeys.every(key => typeof key === "string"));
    assert.equal(new Set(atom.ownDataKeys).size, atom.ownDataKeys.length, `${where}: duplicate own-data key`);
  }
}

export function ownDataKeysProbeDigest(spec, name, pair, misuse, correct, cases) {
  const claim = spec.exports[name];
  if (!atoms(claim.call).length) return undefined;
  validateOwnDataKeys(`${spec.package}#${name}`, claim);
  return createHash("sha256").update(JSON.stringify({
    format: "solid-checker:authored-own-data-keys-probe:v1",
    package: spec.package, version: spec.version, runtime: spec.solidRuntime,
    cases, claim, pair, misuse, correct
  })).digest("hex");
}
