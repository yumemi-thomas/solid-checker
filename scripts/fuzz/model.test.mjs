import { describe, expect, it } from "vitest";
import {
  awaitStmt, button, call, child, component, decl, expr, fn, getOwner, helper, iife, iter, member, nodeCount, obj, param, probe, program, prim,
  read, ref, ret, set, signal, validate, walk, write, num, closedGrammar, contain, show,
} from "./model.mjs";
import { printProgram } from "./print.mjs";

const w = signal("w1", 0, "probe");
const r = signal("r1", 1007, "src");
const k = signal("k1", 0, "sink");
const build = (body, view = [probe("w1")], signals = [w, r, k], helpers = []) => program({ helpers, components: [component("C1", signals, body, view)] });
const writeSite = (value = 17) => write("w1", value, { site: true });

describe("validate", () => {
  it("accepts the ADR 0452 shapes: a returned default-write closure called from a click", () => {
    // createMemo(() => { const f = (x = setW(17)) => x; return f; }) and onClick={() => api()()}
    const p = build([
      decl("m1", prim("memo", fn([decl("f1", fn([ret(ref("p0"))], { params: [param(writeSite())] })), ret(ref("f1"))]))),
    ], [probe("w1"), button("b1", [expr(call(call(ref("m1"))))])]);
    expect(validate(p)).toEqual([]);
    const { text, sites } = printProgram(p);
    expect(text).toContain("const f1 = (p0: number = setW1(17)) => {");
    expect(text).toContain("onClick={() => {");
    expect(text).toContain("m1()();");
    expect(sites).toHaveLength(1);
    expect(text.split("\n")[sites[0].line - 1]).toContain("setW1(17)");
  });
  it("accepts a destructured default with an IIFE and forEach/map inside", () => {
    const p = build([
      decl("m1", prim("memo", fn([
        decl("f1", fn([ret(ref("p0"))], { params: [param(iife(fn([ret(writeSite())])), true)] })),
        decl("f2", fn([ret(ref("p0"))], { params: [param(iter("forEach", fn([expr(writeSite(18))])))] })),
        decl("f3", fn([ret(ref("p0"))], { params: [param(iter("map", fn([ret(writeSite(19))])))] })),
        ret(obj([["f", ref("f1")]])),
      ]))),
    ]);
    expect(validate(p)).toEqual([]);
    expect(printProgram(p).text).toContain("({ p0 = (() => {");
  });
  it("accepts a helper that returns a closure over its callback", () => {
    const p = build([
      decl("m1", prim("memo", fn([ret(call(ref("h1"), [fn([ret(writeSite())])]))]))),
    ], [probe("w1"), button("b1", [expr(call(call(ref("m1"))))])], undefined, [helper("h1", "returnClosure")]);
    expect(validate(p)).toEqual([]);
    expect(printProgram(p).text).toContain("const g = (x: unknown = cb()) => x;");
  });
  it("rejects a reference before its declaration", () => {
    expect(validate(build([expr(call(ref("f1"))), decl("f1", fn([]))]))).toContainEqual(expect.stringContaining("f1 is not declared"));
  });
  it("rejects a compute with no return, and an effect apply that returns a number", () => {
    expect(validate(build([decl("m1", prim("memo", fn([expr(writeSite())])))]))).toContainEqual(expect.stringContaining("compute must return a value"));
    const bad = build([expr(prim("effect", fn([ret(num(1))]), { apply: fn([ret(num(1))]) }))]);
    expect(validate(bad)).toContainEqual(expect.stringContaining("may return only a function"));
    const fine = build([expr(prim("effect", fn([ret(num(1))]), { apply: fn([expr(writeSite())]) }))]);
    expect(validate(fine)).toEqual([]);
  });
  it("rejects what TypeScript rejects: a JSX child that is not a number, onSettled with an async callback", () => {
    expect(validate(build([decl("f1", fn([])), ], [child("v1", ref("f1"))]))).toContainEqual(expect.stringContaining("JSX child must be a number"));
    expect(validate(build([expr(prim("settled", fn([awaitStmt()], { async: true })))]))).toContainEqual(expect.stringContaining("async callback"));
  });
  it("rejects await outside async, a duplicate declaration, an unknown signal and a missing helper argument", () => {
    expect(validate(build([expr(prim("timer", fn([awaitStmt()])))]))).toContainEqual(expect.stringContaining("await outside"));
    expect(validate(build([decl("f1", fn([])), decl("f1", fn([]))]))).toContainEqual(expect.stringContaining("declared twice"));
    expect(validate(build([expr(write("zz", 1))]))).toContainEqual(expect.stringContaining("signal zz"));
    expect(validate(build([expr(call(ref("h1"), []))], undefined, undefined, [helper("h1", "call")]))).toContainEqual(expect.stringContaining("takes a function"));
  });
  it("types owner capture and member access", () => {
    const p = build([
      decl("owner", getOwner()),
      decl("u1", prim("owner", fn([ret(obj([["f", fn([expr(writeSite())])]]))]), { ownerArg: "owner" })),
      expr(call(member(ref("u1"), "f"))),
    ]);
    expect(validate(p)).toEqual([]);
    const missing = build([expr(prim("owner", fn([]), { ownerArg: "owner" }))]);
    expect(validate(missing)).toContainEqual(expect.stringContaining("owner is not captured"));
  });
  it("accepts a sink write only for a number", () => {
    expect(validate(build([expr(prim("settled", fn([expr(set("k1", read("r1", { site: true })))])))]))).toEqual([]);
    expect(validate(build([decl("f1", fn([])), expr(set("k1", ref("f1")))]))).toContainEqual(expect.stringContaining("sink takes a number"));
  });
});

describe("walk", () => {
  it("visits every node once, in file order, with paths that reach it", () => {
    const p = build([expr(writeSite())]);
    const nodes = [...walk(p)];
    expect(nodes.length).toBe(nodeCount(p));
    for (const { node, path } of nodes) expect(path.reduce((value, key) => value[key], p)).toBe(node);
  });
});

describe("closedGrammar with containers (the confirmation of an unexecuted violation)", () => {
  const contained = (kind, extra = {}) => program({ seed: 1, components: [component("C1", [], [contain(kind, 1, [], extra)], [])] });
  it("refuses every container strictly and admits those the text fixes with { containers: true }", () => {
    for (const kind of ["comp-never", "comp-rendered", "comp-slot", "comp-returned", "class-field", "class-static", "class-sblock", "class-method", "class-ctor", "dead-if", "dead-after-return", "dead-fn", "and-true", "and-false"]) {
      expect(closedGrammar(contained(kind)).closed).toBe(false);
      expect(closedGrammar(contained(kind), { containers: true })).toEqual({ closed: true, reasons: [] });
    }
  });

  it("never admits comp-prop: the caller of the component may supply the prop", () => {
    expect(closedGrammar(contained("comp-prop"), { containers: true })).toMatchObject({ closed: false, reasons: ["container comp-prop"] });
  });

  it("admits the rewritten call forms only with containers, and still refuses the rest of the grammar", () => {
    const optional = (name) => expr({ ...call(ref(name)), via: "optional" });
    const via = program({ seed: 1, components: [component("C1", [], [optional("f")], [])] });
    const declared = program({ seed: 1, components: [component("C1", [], [decl("f", fn([])), optional("f")], [])] });
    expect(closedGrammar(declared).reasons).toContain("call through optional");
    expect(closedGrammar(declared, { containers: true }).closed).toBe(true);
    expect(closedGrammar(via, { containers: true }).reasons).toContain("call of undeclared f");
    const shown = program({ seed: 1, components: [component("C1", [], [], [show(read("s1"), [])])] });
    expect(closedGrammar(shown, { containers: true }).closed).toBe(false);
  });
});
