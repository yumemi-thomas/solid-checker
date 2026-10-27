// Deduplication by construct path.
//
// Two findings are the same finding when the same rule reports the same *shape of
// code*: the chain of constructs from the component down to the site (a memo compute,
// a named arrow closure, a destructured default, an IIFE, a write ...) plus what the
// program does with each closure on that chain (called in place, returned, stored,
// passed to a helper of some form, called later from a handler). Names, signal
// identities, literals, line numbers and the rest of the file play no part, so a
// hundred programs that differ only in decoys collapse into one entry, and a shrunk
// repro has the same signature as the program it came from whenever shrinking kept the
// construct. (Solid's semantic fuzzer dedupes on a work-ledger footprint of an engine
// run, packages/signals/tests/semantics/footprints.ts in PR #3446; a checker finding has
// no such ledger, so the construct path takes its place.)
import { getAt, slotsOf, walk } from "./model.mjs";

/** Parent links for every node: Map<node, { parent, key }>. */
function parents(root) {
  const map = new Map();
  for (const { node } of walk(root)) {
    for (const [key, isList] of slotsOf(node)) {
      const children = isList ? node[key] : [node[key]];
      for (const child of children) map.set(child, { parent: node, key });
    }
  }
  return map;
}

function describe(node, helperForms) {
  switch (node.k) {
    case "fn": return `fn:${node.form}${node.async ? ":async" : ""}`;
    case "param": return `param:${node.destructured ? "destructured" : "plain"}`;
    case "prim": return `prim:${node.name}${node.name === "owner" ? `:${node.ownerArg ?? "null"}` : ""}`;
    case "iter": return `iter:${node.method}`;
    case "call": return node.callee.k === "ref" && helperForms.has(node.callee.name) ? `call:helper:${helperForms.get(node.callee.name)}` : node.via ? `call:${node.via}` : "call";
    case "own": return node.site ? null : `own:${node.op}${node.handle ? ":handle" : ""}`;
    case "contain": return `contain:${node.kind}${node.use ? `:${node.use}` : ""}${node.async ? ":async" : ""}`;
    case "try": case "flush": return node.k;
    case "iife": case "obj": case "field": case "button": case "child": case "attr": case "show": case "for": case "return": return node.k;
    default: return null;
  }
}

/** The scope a reference sits in: the nearest enclosing host construct. */
function scopeOf(node, up) {
  for (let at = up.get(node)?.parent; at; at = up.get(at)?.parent) {
    if (at.k === "prim") return at.name;
    if (at.k === "button") return "handler";
    if (at.k === "child" || at.k === "attr") return "jsx";
    if (at.k === "fn" && at.async) return "async";
    if (at.k === "component") return "body";
  }
  return "body";
}

/** What a reference to a bound name does, from its immediate context. */
function roleOf(refNode, up, helperForms) {
  const link = up.get(refNode);
  if (!link) return "used";
  const { parent, key } = link;
  if (parent.k === "call") {
    if (key === "callee") return "called";
    if (parent.callee.k === "ref" && helperForms.has(parent.callee.name)) return `passed:${helperForms.get(parent.callee.name)}`;
    return "argument";
  }
  if (parent.k === "return") return "returned";
  if (parent.k === "field") return "stored";
  if (parent.k === "prim") return `given:${parent.name}`;
  if (parent.k === "member") return "member";
  if (parent.k === "decl") return "alias";
  return "used";
}

/**
 * The construct path of the node at `path`: ancestor tokens plus, for every ancestor
 * bound to a name, the roles and scopes of that name's references.
 * @returns {{ chain: string[], uses: string[] }}
 */
export function constructPath(program, path) {
  const helperForms = new Map((program.helpers ?? []).map((helper) => [helper.name, helper.form]));
  const up = parents(program);
  const chain = [];
  const uses = [];
  // Ancestors from the root down to (and including) the site.
  const ancestors = [];
  let node = program;
  ancestors.push(node);
  for (const key of path) { node = node[key]; if (node && typeof node === "object" && typeof node.k === "string") ancestors.push(node); }
  const component = ancestors.find((a) => a.k === "component");
  for (const ancestor of ancestors) {
    const token = describe(ancestor, helperForms);
    if (token) chain.push(token);
    else if (ancestor.k === "component") chain.push(`component.${path[2]}`);
    else if (ancestor.k === "read" || ancestor.k === "write") chain.push(`site:${ancestor.k}${ancestor.via ? `:${ancestor.via}` : ""}`);
    else if (ancestor.k === "own") chain.push(`site:own:${ancestor.op}`);
    else if (ancestor.k === "use") chain.push(`site:use:${ancestor.code}:${ancestor.bind}`);
    // Names bound by a declaration: how does the rest of the component use them?
    const link = up.get(ancestor);
    if (link?.parent.k === "decl" && link.key === "e") {
      const name = link.parent.name;
      const scopeRoot = component ?? program;
      const roles = [];
      for (const { node: candidate } of walk(scopeRoot)) {
        if (candidate.k === "ref" && candidate.name === name) roles.push(`${roleOf(candidate, up, helperForms)}@${scopeOf(candidate, up)}`);
      }
      uses.push(`${describe(ancestor, helperForms) ?? ancestor.k}=${[...new Set(roles)].sort().join("+") || "unused"}`);
    }
  }
  return { chain, uses };
}

/** The identity of a finding: rule, claim and construct path. Everything else about the program is noise. */
export function signature({ rule, kind, program, path, outcome }) {
  const { chain, uses } = constructPath(program, path);
  return [`${rule}`, kind, outcome ?? "", chain.join(">"), uses.join(",")].join("|");
}

/** Group findings by signature; the representative of a group is its smallest program (fewest nodes). */
export function dedupe(entries) {
  const groups = new Map();
  for (const entry of entries) {
    const group = groups.get(entry.signature);
    if (!group) groups.set(entry.signature, { signature: entry.signature, count: 1, representative: entry, members: [entry.id] });
    else {
      group.count += 1;
      group.members.push(entry.id);
      if ((entry.size ?? Infinity) < (group.representative.size ?? Infinity)) group.representative = entry;
    }
  }
  return [...groups.values()].sort((a, b) => b.count - a.count || a.signature.localeCompare(b.signature));
}

export { getAt };
