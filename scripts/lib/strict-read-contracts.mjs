// Authored operation assertions and their source citations.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";

function memberCalls(call) {
  return (call?.operations ?? []).flatMap(operation => {
    const output = operation.output;
    const entries = output?.kind === "tuple" ? (output.items ?? []).map((value, index) => [String(index), value])
      : output?.kind === "object" ? Object.entries(output.properties ?? {})
      : output?.kind === "returned-callable" ? (output.members ?? []).map(member => [member.name, member.value]) : [];
    return entries.filter(([, value]) => value?.kind === "effectful-callable")
      .map(([key, value]) => [`${operation.id}.${key}`, value.call]);
  });
}

function returnedCalls(call) {
  return (call?.operations ?? []).filter(operation => operation.output?.kind === "returned-callable" && operation.output.call)
    .map(operation => [operation.id, operation.output.call]);
}

export function hasStrictReadAssertion(call) {
  return (call?.operations ?? []).some(operation => Object.hasOwn(operation, "strictRead"))
    || memberCalls(call).some(([, member]) => hasStrictReadAssertion(member))
    || returnedCalls(call).some(([, returned]) => hasStrictReadAssertion(returned));
}

export function validateStrictReads(where, claim) {
  const visit = (call, closures, location) => {
    for (const operation of call?.operations ?? []) {
      if (!Object.hasOwn(operation, "strictRead")) continue;
      assert.equal(operation.strictRead, "cleared", `${location}: unknown strictRead assertion`);
      assert.equal(operation.kind, "read", `${location}: strictRead is valid only on a read`);
      assert.equal(operation.tracking, "untracked", `${location}: strictRead cleared requires untracked`);
      const cites = value => typeof value === "string" && /\S+:\d+/.test(value);
      assert(cites(operation.why) || cites(closures?.reads),
        `${location} operation ${operation.id}: strictRead cleared needs a source citation in why or closures.reads`);
    }
    for (const [key, member] of memberCalls(call))
      visit(member, claim.memberClosures?.[key], `${location} member ${key}`);
    for (const [key, returned] of returnedCalls(call))
      visit(returned, claim.returnedClosures?.[key], `${location} returned ${key}`);
  };
  visit(claim.call, claim.closures, where);
}

export function strictReadWireCall(call) {
  const result = structuredClone(call);
  const visit = graph => {
    for (const operation of graph?.operations ?? []) delete operation.why;
    for (const [, member] of memberCalls(graph)) visit(member);
    for (const [, returned] of returnedCalls(graph)) visit(returned);
  };
  visit(result);
  return result;
}

export function strictReadProbeDigest(spec, name, pair, misuse, correct, cases) {
  const claim = spec.exports[name];
  if (!hasStrictReadAssertion(claim.call)) return undefined;
  validateStrictReads(`${spec.package}#${name}`, claim);
  return createHash("sha256").update(JSON.stringify({
    format: "solid-checker:authored-strict-read-probe:v1",
    package: spec.package, version: spec.version, runtime: spec.solidRuntime,
    cases, claim, pair, misuse, correct
  })).digest("hex");
}
