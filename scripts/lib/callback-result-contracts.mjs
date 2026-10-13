// Authored result censuses: omissions keep their old probe/result identity.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";

function calls(call) {
  const result = [call];
  for (const operation of call?.operations ?? []) {
    const output = operation.output;
    if (output?.kind === "returned-callable" && output.call) result.push(...calls(output.call));
    const entries = output?.kind === "tuple" ? (output.items ?? [])
      : output?.kind === "object" ? (Array.isArray(output.properties) ? output.properties.map(p => p.value) : Object.values(output.properties ?? {}))
      : output?.kind === "returned-callable" ? (output.members ?? []).map(p => p.value) : [];
    for (const member of entries) if (member?.kind === "effectful-callable") result.push(...calls(member.call));
  }
  return result;
}

export function validateCallbackResults(where, claim) {
  const graphs = calls(claim.call);
  const entries = graphs.flatMap(call => (call?.callbackResults ?? []).map(result => [call, result]));
  const cited = claim.resultClosures ?? {};
  const producers = new Set(entries.map(([, result]) => result.producer));
  for (const id of Object.keys(cited)) assert(producers.has(id), `${where}: resultClosures names no result producer ${id}`);
  for (const [call, result] of entries) {
    const row = (call.callbacks ?? []).filter(row => row.operation === result.producer);
    assert.equal(row.length, 1, `${where}: result needs one producer invocation`);
    assert(Number.isInteger(row[0].from.arg), `${where}: recursive result producer unsupported`);
    assert(result.shape !== undefined, `${where}: result needs its own shape`);
    assert((result.closed ?? []).every(domain => domain === "uses"), `${where}: result closes only uses`);
    const closed = (result.closed ?? []).includes("uses");
    if (closed) assert(typeof cited[result.producer] === "string" && /\S+:\d+/.test(cited[result.producer]), `${where}: result closure needs installed source citation`);
    else assert(cited[result.producer] === undefined, `${where}: open result cannot carry closure citation`);
    if (Array.isArray(result.uses) && result.uses.length === 0) assert(closed, `${where}: empty open result census unsupported`);
    for (const id of result.callableOnly ?? []) {
      assert((result.uses ?? []).includes(id), `${where}: callableOnly names a described result use`);
      const operation = (call.operations ?? []).find(operation => operation.id === id);
      assert(operation?.kind === "invoke" && (!operation.protocol || operation.protocol === "call") && operation.count?.min === 0,
        `${where}: callableOnly needs an optional call and its exact runtime callable test`);
    }
    for (const id of result.uses ?? []) {
      const uses = (call.callbacks ?? []).filter(row => row.operation === id);
      assert.equal(uses.length, 1);
      assert.equal(uses[0].from.operation, result.producer);
      assert((call.edges ?? []).some(edge => edge.kind === "data" && edge.from === result.producer && edge.to === id), `${where}: missing producer data edge`);
    }
  }
}

export function callbackResultProbeDigest(spec, name, pair, misuse, correct, cases) {
  const claim = spec.exports[name];
  if (!calls(claim.call).some(call => (call?.callbackResults ?? []).length)) return undefined;
  validateCallbackResults(`${spec.package}#${name}`, claim);
  return createHash("sha256").update(JSON.stringify({
    format: "solid-checker:authored-callback-results-probe:v1",
    package: spec.package, version: spec.version, runtime: spec.solidRuntime,
    cases, claim, pair, misuse, correct
  })).digest("hex");
}
