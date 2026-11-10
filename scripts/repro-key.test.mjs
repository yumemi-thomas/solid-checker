import assert from "node:assert/strict";
import { test } from "vitest";

import { describeKey, holdsKey, parseKey } from "./lib/repro-key.mjs";

// ADR 0956: what scripts/repro-reduce.mjs holds while it reduces: the typed blocker of ADR 0947, or the original text key.
const none = { rule: null, family: null, context: "", marker: null, blocker: null, formula: null, fired: null, missing: null };
const target = "/work/app/src/a.ts";
const lines = ["", "", "", "  patch(Number, key, f);", "  setX(1);"];
const at = (line, path = target) => ({ path, line, endLine: line });
const veto = (fired, extra = {}) => ({ kind: "uncertifiable", id: "SC9012", primaryLocation: at(4), message: "m", analysisContext: "standard-member-impostor", blockedBy: { kind: "standard-member-impostor", scope: "program", formula: "handed-root-patch", fired, ...extra } });

test("the text key needs a rule, a family and a marker, and nothing typed", () => {
  assert.throws(() => parseKey(none), /--rule is required/);
  assert.throws(() => parseKey({ ...none, rule: "SC2001", family: "unplaced-write" }), /--marker is required/);
  assert.throws(() => parseKey({ ...none, rule: "SC2001", family: "x", marker: "m", formula: "f" }), /--formula needs --blocker/);
  assert.deepEqual(parseKey({ ...none, rule: "SC2001", family: "unplaced-write", marker: "setX" }), { kind: "text", rule: "SC2001", family: "unplaced-write", context: "", marker: "setX" });
});

test("the typed key names a blocker kind, a veto's formula and exact fired set, or a missing fact", () => {
  assert.deepEqual(parseKey({ ...none, blocker: "function-run-unplaced" }), { kind: "typed", blocker: "function-run-unplaced", formula: null, fired: null, missing: null, rule: null, marker: null });
  const key = parseKey({ ...none, blocker: "standard-member-impostor", formula: "handed-root-patch", fired: "StandardRootHanded, OpenMemberWrite(*)", rule: "SC9012" });
  assert.deepEqual([key.formula, key.fired, key.rule], ["handed-root-patch", ["OpenMemberWrite(*)", "StandardRootHanded"], "SC9012"]);
  assert.deepEqual(parseKey({ ...none, blocker: "provider-not-proven-above", missing: "provider-above:Ctx" }).missing, { fact: "provider-above", member: "Ctx" });
  assert.deepEqual(parseKey({ ...none, blocker: "provider-not-proven-above", missing: "provider-above" }).missing, { fact: "provider-above", member: null });
});

test("a mixed or half-named key is refused", () => {
  assert.throws(() => parseKey({ ...none, blocker: "k", family: "unplaced-write" }), /belongs to the text key/);
  assert.throws(() => parseKey({ ...none, blocker: "k", context: "c" }), /belongs to the text key/);
  assert.throws(() => parseKey({ ...none, blocker: "k", formula: "f" }), /--formula needs --fired/);
  assert.throws(() => parseKey({ ...none, blocker: "k", fired: "a" }), /--fired needs --formula/);
  assert.throws(() => parseKey({ ...none, blocker: "k", formula: "f", fired: "a", missing: "x" }), /two kinds of blocker/);
  assert.throws(() => parseKey({ ...none, blocker: "k", missing: ":m" }), /names a fact/);
});

test("a veto is held by its formula and exactly the hazards that fired, in any order", () => {
  const key = parseKey({ ...none, blocker: "standard-member-impostor", formula: "handed-root-patch", fired: "OpenMemberWrite(*),StandardRootHanded" });
  assert.equal(holdsKey(key, veto(["StandardRootHanded", "OpenMemberWrite(*)"]), target, lines), true);
  // one hazard fewer or more is another veto, never held
  assert.equal(holdsKey(key, veto(["OpenMemberWrite(*)"]), target, lines), false);
  assert.equal(holdsKey(key, veto(["OpenMemberWrite(*)", "StandardRootHanded", "RootPatch"]), target, lines), false);
  assert.equal(holdsKey(key, { ...veto(["OpenMemberWrite(*)", "StandardRootHanded"]), blockedBy: { ...veto(["OpenMemberWrite(*)", "StandardRootHanded"]).blockedBy, formula: "impostor-veto" } }, target, lines), false);
});

test("a result is held only when it is an uncertifiable finding of the target file", () => {
  const key = parseKey({ ...none, blocker: "standard-member-impostor" });
  assert.equal(holdsKey(key, veto(["A"]), target, lines), true);
  assert.equal(holdsKey(key, { ...veto(["A"]), kind: "violation" }, target, lines), false);
  assert.equal(holdsKey(key, { ...veto(["A"]), primaryLocation: at(4, "/work/app/src/b.ts") }, target, lines), false);
  assert.equal(holdsKey(key, { ...veto(["A"]), blockedBy: undefined }, target, lines), false);
  assert.equal(holdsKey(key, { ...veto(["A"]), blockedBy: { kind: "function-run-unplaced" } }, target, lines), false);
  // the kind alone is any row of that kind, with or without a formula
  assert.equal(holdsKey(key, { ...veto(["A"]), blockedBy: { kind: "standard-member-impostor" } }, target, lines), true);
});

test("a rule and a marker narrow a typed key", () => {
  const key = parseKey({ ...none, blocker: "standard-member-impostor", rule: "SC9012", marker: "patch(" });
  assert.equal(holdsKey(key, veto(["A"]), target, lines), true);
  assert.equal(holdsKey(key, { ...veto(["A"]), id: "SC2001" }, target, lines), false);
  assert.equal(holdsKey(key, { ...veto(["A"]), primaryLocation: at(5) }, target, lines), false);
});

test("a missing fact is held by its fact, and by its member when one is named", () => {
  const row = (member) => ({ kind: "uncertifiable", id: "SC4002", primaryLocation: at(4), blockedBy: { kind: "provider-not-proven-above", missing: { fact: "provider-above", ...(member ? { member } : {}) } } });
  const any = parseKey({ ...none, blocker: "provider-not-proven-above", missing: "provider-above" });
  const named = parseKey({ ...none, blocker: "provider-not-proven-above", missing: "provider-above:Ctx" });
  assert.equal(holdsKey(any, row("Ctx"), target, lines), true);
  assert.equal(holdsKey(any, row(), target, lines), true);
  assert.equal(holdsKey(named, row("Ctx"), target, lines), true);
  assert.equal(holdsKey(named, row("Other"), target, lines), false);
  assert.equal(holdsKey(named, row(), target, lines), false);
  assert.equal(holdsKey(parseKey({ ...none, blocker: "provider-not-proven-above", missing: "other-fact" }), row("Ctx"), target, lines), false);
});

test("the text key is as it was: rule, message family, context kind and a marker on the line", () => {
  const key = parseKey({ ...none, rule: "SC2001", family: "unplaced-write", context: "some-context", marker: "setX" });
  const finding = { kind: "uncertifiable", id: "SC2001", primaryLocation: at(5), message: "setter setX is called, but no compiler or semantic fact places where this code runs", analysisContext: "some-context" };
  assert.equal(holdsKey(key, finding, target, lines), true);
  assert.equal(holdsKey(key, { ...finding, id: "SC2003" }, target, lines), false);
  assert.equal(holdsKey(key, { ...finding, message: "something else" }, target, lines), false);
  assert.equal(holdsKey(key, { ...finding, primaryLocation: at(4) }, target, lines), false);
});

test("a key is described on one line for the log", () => {
  assert.equal(describeKey(parseKey({ ...none, blocker: "standard-member-impostor", formula: "handed-root-patch", fired: "B,A", rule: "SC9012" })), "standard-member-impostor handed-root-patch [A, B] SC9012");
  assert.equal(describeKey(parseKey({ ...none, blocker: "k", missing: "f:m" })), "k missing f:m");
  assert.equal(describeKey(parseKey({ ...none, rule: "SC2001", family: "unplaced-write", marker: "setX" })), 'SC2001 unplaced-write on a line with "setX"');
});
