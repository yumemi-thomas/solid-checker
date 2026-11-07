import { describe, expect, it } from "vitest";
import { callStarts, frameStarts } from "./call-starts.mjs";

// Review 4c: V8 puts a member call's frame at the property; a checker row starts at the call or its callee object.
describe("callStarts", () => {
  const starts = callStarts("const a = providers.map((p) => p.x);\nlog(st.x); new Foo(1); <Child v={c} />;\nr();");
  it("maps a frame in a call's callee to the call's start and its callee object's", () => {
    expect(starts(1, 21)).toEqual([{ line: 1, column: 21 }, { line: 1, column: 11 }]);
    expect(starts(2, 17)).toEqual([{ line: 2, column: 17 }, { line: 2, column: 12 }, { line: 2, column: 16 }]);
    expect(starts(2, 25)).toEqual([{ line: 2, column: 25 }, { line: 2, column: 24 }]);
  });
  it("keeps any other frame at its own position: a position in a call's arguments is not the call", () => {
    expect(starts(2, 5)).toEqual([{ line: 2, column: 5 }]);
    expect(starts(1, 33)).toEqual([{ line: 1, column: 33 }]);
    expect(starts(3, 1)).toEqual([{ line: 3, column: 1 }]);
  });
  it("maps a frame on a member access's property, or on `delete`, to the access (V8 places a trap's frame there)", () => {
    const at = callStarts("s2.x = 5;\ndelete st.a;\nv = st.a.b;\nw = (st as any).c;\nu = st.items[0];\nu = f(k)[k2];");
    expect(at(1, 4)).toContainEqual({ line: 1, column: 1 });
    expect(at(2, 1)).toContainEqual({ line: 2, column: 8 });
    expect(at(3, 10)).toContainEqual({ line: 3, column: 5 });
    expect(at(4, 17)).toEqual(expect.arrayContaining([{ line: 4, column: 5 }, { line: 4, column: 6 }]));
    expect(at(5, 14)).toContainEqual({ line: 5, column: 5 });
    expect(at(6, 11)).toContainEqual({ line: 6, column: 5 });
  });
  it("keeps an access's object where it is: a position on the object is not the access", () => {
    const at = callStarts("v = st.x;");
    expect(at(1, 5)).toEqual([{ line: 1, column: 5 }]);
    expect(at(1, 8)).toEqual([{ line: 1, column: 8 }, { line: 1, column: 5 }]);
  });
  it("takes a read past a call's `)` (no mapping after it), a member key, and a for...of binding to the expression", () => {
    const at = callStarts("v = use().x;\nw = o[E.A]();\nfor (const a of g()) log(a);");
    expect(at(1, 9)).toContainEqual({ line: 1, column: 5 });
    expect(at(2, 9)).toEqual(expect.arrayContaining([{ line: 2, column: 7 }, { line: 2, column: 5 }]));
    expect(at(3, 12)).toContainEqual({ line: 3, column: 17 });
  });
  it("gives a call's arguments only to a top frame", () => {
    const frame = frameStarts("  [1].forEach(setC);\n  JSON.stringify({ x: st });");
    expect(frame({ line: 1, column: 8 }, 0)).toContainEqual({ line: 1, column: 15, handed: true, index: 0, noTarget: true });
    expect(frame({ line: 1, column: 8 }, 1)).toEqual(expect.not.arrayContaining([expect.objectContaining({ column: 15 })]));
    expect(frame({ line: 2, column: 8 }, 0)).toContainEqual({ line: 2, column: 18, handed: true, index: 0, noTarget: true });
  });
  it("says a frame mounts by position: another tag on the line is not this frame", () => {
    const at = callStarts("function C(p: any) { log(p.a); return <Child a={1} />; }\nrender(() => <App />, el);\nconst x = <div />;");
    expect(at.mount(1, 40)).toBe(true);
    expect(at.mount(1, 28)).toBe(false);
    expect(at.mount(2, 1)).toBe(true);
    expect(at.mount(2, 14)).toBe(true);
    expect(at.mount(3, 11)).toBe(false);
  });
  it("takes a spread's operand to the spread, a template to its substitutions, a binding to its pattern", () => {
    const at = frameStarts("  [...s1];\n  `${s1}`;");
    expect(at({ line: 1, column: 7 }, 1)).toContainEqual({ line: 1, column: 4 });
    expect(at({ line: 2, column: 3 }, 0)).toContainEqual({ line: 2, column: 6 });
  });
  it("gives no Solid primitive's or setter's arguments", () => {
    const frame = frameStarts('import { untrack, createSignal } from "solid-js";\nconst [c, setC] = createSignal(0);\nuntrack(c);\nsetC(1);');
    expect(frame({ line: 3, column: 1 }, 0)).toEqual(expect.not.arrayContaining([expect.objectContaining({ handed: true })]));
    expect(frame({ line: 4, column: 1 }, 0)).toEqual(expect.not.arrayContaining([expect.objectContaining({ handed: true })]));
  });
  it("marks a dependency's arguments as no store write's target, a project function's as open, Object.assign's as open", () => {
    const frame = frameStarts('import { writeHeld } from "fz-pkg";\nimport { pick } from "./lib";\nwriteHeld(a);\npick(a);\nObject.assign(a, b);');
    expect(frame({ line: 3, column: 1 }, 0)).toContainEqual(expect.objectContaining({ noTarget: true, handed: true }));
    expect(frame({ line: 4, column: 1 }, 0)).toContainEqual(expect.objectContaining({ noTarget: false, handed: true, index: 0 }));
    expect(frame({ line: 5, column: 8 }, 0)).toContainEqual(expect.objectContaining({ noTarget: false, handed: true, index: 0 }));
  });
  it("takes a destructuring's binding to the pattern", () => {
    const at = callStarts("const { a, b } = merge(st);\nconst [c] = xs;");
    expect(at(1, 9)).toContainEqual({ line: 1, column: 7 });
    expect(at(1, 12)).toContainEqual({ line: 1, column: 7 });
    expect(at(2, 8)).toContainEqual({ line: 2, column: 7 });
  });
  it("takes a nested binding and an assignment pattern's property to the whole pattern", () => {
    const at = callStarts("const [{ v }] = xs;\n({ w } = st);");
    expect(at(1, 10)).toContainEqual({ line: 1, column: 7 });
    expect(at(2, 4)).toContainEqual({ line: 2, column: 2 });
  });
});
