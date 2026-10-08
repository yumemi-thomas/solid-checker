import assert from "node:assert/strict";
import { createHash } from "node:crypto";

function graphs(claim) {
  return (claim.call?.operations ?? []).filter(operation =>
    operation.output?.kind === "returned-callable" && operation.output.call)
    .map(operation => [operation.id, operation.output.call]);
}

export function validateCaptures(where, claim) {
  assert(!(claim.call?.captures ?? []).length, `${where}: factory call cannot carry captures`);
  const cited = claim.captureClosures ?? {};
  const keys = new Set();
  for (const [returned, graph] of graphs(claim)) {
    const ids = new Set();
    for (const capture of graph.captures ?? []) {
      assert(typeof capture.id === "string" && capture.id.length && !ids.has(capture.id), `${where}: distinct capture id required`);
      ids.add(capture.id);
      const key = `${returned}.${capture.id}`;
      keys.add(key);
      assert(typeof cited[key] === "string" && /\S+:\d+/.test(cited[key]), `${where}: capture ${key} needs installed source citation`);
      const source = capture.from ?? {};
      assert.equal([source.arg !== undefined, source.resource !== undefined, source.operation !== undefined].filter(Boolean).length, 1,
        `${where}: capture needs exactly one factory source`);
      assert(source.capture === undefined && source.members === undefined, `${where}: recursive/member-class capture unsupported`);
      if (source.arg !== undefined) assert(Number.isInteger(source.arg) && source.arg >= 0 && source.arg <= 65535);
      if (source.resource !== undefined) assert((claim.call.resources ?? []).some(resource => resource.id === source.resource), `${where}: missing factory resource`);
      if (source.operation !== undefined) assert(source.operation !== returned && (claim.call.operations ?? []).some(operation => operation.id === source.operation), `${where}: missing factory result`);
      assert((source.path ?? []).every(key => typeof key === "string" && key.length && key !== "*"), `${where}: exact capture path required`);
    }
    for (const callback of graph.callbacks ?? []) {
      if (callback.from?.capture === undefined) continue;
      assert(ids.has(callback.from.capture), `${where}: unknown capture source`);
      assert(["arg", "resource", "operation", "members"].every(key => callback.from[key] === undefined), `${where}: ambiguous capture source`);
      assert((callback.from.path ?? []).every(key => typeof key === "string" && key.length && key !== "*"));
    }
  }
  for (const key of Object.keys(cited)) assert(keys.has(key), `${where}: captureClosures names no capture ${key}`);
}

export function captureProbeDigest(spec, name, pair, misuse, correct, cases) {
  const claim = spec.exports[name];
  if (!graphs(claim).some(([, graph]) => (graph.captures ?? []).length)) return undefined;
  validateCaptures(`${spec.package}#${name}`, claim);
  return createHash("sha256").update(JSON.stringify({
    format: "solid-checker:authored-captures-probe:v1",
    package: spec.package, version: spec.version, runtime: spec.solidRuntime,
    cases, claim, pair, misuse, correct
  })).digest("hex");
}
