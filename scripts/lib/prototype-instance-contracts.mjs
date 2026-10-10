import assert from "node:assert/strict";
import { createHash } from "node:crypto";

function recipes(call) {
  return (call?.operations ?? []).filter(operation => operation.output?.kind === "prototype-instance");
}

export function validatePrototypeInstances(where, claim) {
  const cited = claim.prototypeClosures ?? {};
  const addresses = new Set();
  for (const operation of recipes(claim.call)) {
    assert.equal(operation.kind, "return", `${where}: prototype recipe requires a constructor return`);
    assert(["opaque", "values", "entries"].includes(operation.output.population), `${where}: constructor population must be stated`);
    const members = operation.output.members;
    assert(Array.isArray(members) && members.length, `${where}: prototype members must be stated`);
    const names = new Set();
    for (const member of members) {
      assert(typeof member.name === "string" && member.name.length, `${where}: member needs an exact key`);
      assert(!names.has(member.name), `${where}: duplicate prototype member ${member.name}`);
      names.add(member.name);
      assert(["method", "getter", "iterator"].includes(member.kind), `${where}: unsupported prototype operation`);
      assert(member.name !== "@@iterator" || member.kind === "iterator", `${where}: @@iterator resumes a generator`);
      assert(Array.isArray(member.tracks) && member.tracks.length, `${where}: missing TriggerCache census`);
      for (const track of member.tracks) {
        assert(typeof track.cache === "string" && track.cache.length, `${where}: cache identity required`);
        assert((Number.isInteger(track.argument) && track.argument >= 0 && track.argument <= 65535)
          !== (typeof track.shared === "string" && track.shared.length > 0), `${where}: one exact key source required`);
        assert(member.kind === "method" || track.argument === undefined, `${where}: getter/iterator cannot consume a method argument`);
      }
      const address = `${operation.id}.${member.name}`;
      addresses.add(address);
      assert(typeof cited[address] === "string" && /\S+:\d+/.test(cited[address]),
        `${where}: prototype behavior needs installed source citation ${address}`);
    }
  }
  for (const address of Object.keys(cited)) assert(addresses.has(address), `${where}: citation names no prototype member ${address}`);
}

export function prototypeInstanceProbeDigest(spec, name, pair, misuse, correct, cases) {
  const claim = spec.exports[name];
  if (!recipes(claim.call).length) return undefined;
  validatePrototypeInstances(`${spec.package}#${name}`, claim);
  return createHash("sha256").update(JSON.stringify({
    format: "solid-checker:authored-prototype-instance-probe:v1",
    package: spec.package, version: spec.version, runtime: spec.solidRuntime,
    cases, claim, pair, misuse, correct
  })).digest("hex");
}
