import { describe, expect, it } from "vitest";
import { dedupe, constructPath, signature } from "./dedupe.mjs";
import { button, call, component, num, decl, expr, fn, helper, iife, obj, param, probe, prim, program, ref, ret, signal, write } from "./model.mjs";
import { printProgram } from "./print.mjs";

const comp = (name, sig, body, view, helpers = []) => program({ helpers, components: [component(name, [signal(sig, 0, "probe")], body, [probe(sig), ...view])] });
const site = (sig, value) => write(sig, value, { site: true });
const siteOf = (p) => printProgram(p).sites[0];
const sigOf = (p, rule = "no-write-in-pure-computation") => signature({ rule, kind: "candidate-false-positive", program: p, path: siteOf(p).path, outcome: "silent" });

// createMemo(() => { const f = (p0 = <write>) => p0; <disposition> })
const memoWithDefault = (name, sig, value, tail, view = []) => comp(name, sig, [
  decl("m1", prim("memo", fn([decl("f1", fn([ret(ref("p0"))], { params: [param(site(sig, value))] })), ...tail]))),
], view);

describe("signature", () => {
  it("ignores names, values and the rest of the file", () => {
    const a = memoWithDefault("C1", "w1", 17, [ret(ref("f1"))], [button("b1", [expr(call(call(ref("m1"))))])]);
    const b = memoWithDefault("Other", "zz9", 4242, [ret(ref("f1"))], [button("b7", [expr(call(call(ref("m1"))))])]);
    expect(sigOf(a)).toBe(sigOf(b));
  });

  it("tells a closure returned and called later from one called in place", () => {
    const returned = memoWithDefault("C1", "w1", 17, [ret(ref("f1"))], [button("b1", [expr(call(call(ref("m1"))))])]);
    const calledNow = memoWithDefault("C1", "w1", 17, [expr(call(ref("f1"))), ret(num(1))]);
    expect(sigOf(returned)).not.toBe(sigOf(calledNow));
    expect(constructPath(returned, siteOf(returned).path).uses.join()).toMatch(/fn:arrow=returned@memo/);
    expect(constructPath(calledNow, siteOf(calledNow).path).uses.join()).toMatch(/fn:arrow=called@memo/);
    expect(constructPath(returned, siteOf(returned).path).uses.join()).toMatch(/prim:memo=called@handler/);
  });

  it("tells a plain default from a destructured one, and an IIFE layer from none", () => {
    const plain = comp("C1", "w1", [decl("f1", fn([], { params: [param(site("w1", 1))] }))], []);
    const destructured = comp("C1", "w1", [decl("f1", fn([], { params: [param(site("w1", 1), true)] }))], []);
    const wrapped = comp("C1", "w1", [decl("f1", fn([], { params: [param(iife(fn([ret(site("w1", 1))])))] }))], []);
    expect(new Set([sigOf(plain), sigOf(destructured), sigOf(wrapped)]).size).toBe(3);
  });

  it("separates helper forms and the rule and the claim", () => {
    const viaHelper = (form) => comp("C1", "w1", [decl("m1", prim("memo", fn([ret(call(ref("h1"), [fn([ret(site("w1", 1))])]))])))], [], [helper("h1", form)]);
    expect(sigOf(viaHelper("returnClosure"))).not.toBe(sigOf(viaHelper("call")));
    const p = viaHelper("call");
    expect(sigOf(p, "strict-read-untracked")).not.toBe(sigOf(p));
    expect(sigOf(p)).toContain("call:helper:call");
  });

  it("records a stored closure as stored", () => {
    const stored = comp("C1", "w1", [decl("m1", prim("memo", fn([decl("f1", fn([expr(site("w1", 1))])), ret(obj([["f", ref("f1")]]))])))], []);
    expect(constructPath(stored, siteOf(stored).path).uses.join()).toMatch(/fn:arrow=stored@memo/);
  });
});

describe("dedupe", () => {
  it("groups by signature, counts, and keeps the smallest program as the representative", () => {
    const groups = dedupe([
      { id: "a", signature: "S1", size: 40 }, { id: "b", signature: "S2", size: 10 }, { id: "c", signature: "S1", size: 22 }, { id: "d", signature: "S1", size: 30 },
    ]);
    expect(groups.map((g) => [g.signature, g.count, g.representative.id])).toEqual([["S1", 3, "c"], ["S2", 1, "b"]]);
    expect(groups[0].members).toEqual(["a", "c", "d"]);
  });
});
