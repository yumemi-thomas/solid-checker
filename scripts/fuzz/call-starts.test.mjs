import { describe, expect, it } from "vitest";
import { callStarts, frameStarts, statementAt } from "./call-starts.mjs";

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
  it("takes the start of a statement whose whole operand is a global read to the read (V8 gives it no position of its own)", () => {
    const source = "function f() {\n  void devicePixelRatio;\n  void typeof devicePixelRatio;\n  if (devicePixelRatio) {}\n  return (devicePixelRatio as number);\n}\n";
    const starts = callStarts(source);
    expect(starts(2, 3)).toContainEqual({ line: 2, column: 8 });
    expect(starts(3, 3)).toContainEqual({ line: 3, column: 15 });
    expect(starts(4, 3)).toContainEqual({ line: 4, column: 7 });
    expect(starts(5, 3)).toContainEqual({ line: 5, column: 11 });
    // A read that is an argument, an initializer or an element has its frame on the identifier: nothing moves, and a statement of another shape is not this.
    const other = callStarts("function f() {\n  use(devicePixelRatio);\n  const r = devicePixelRatio;\n  void [devicePixelRatio];\n  void devicePixelRatio + 1;\n}\n");
    for (const line of [2, 3, 4, 5]) expect(other(line, 3)).toEqual([{ line, column: 3 }]);
  });
  it("takes a frame on an attribute's name to the one call inside it; of two, to neither (R24)", () => {
    const at = callStarts("<C run={() => n()} other={m()} />;\n<C run={() => a() + b()} />;");
    expect(at(1, 4)).toContainEqual({ line: 1, column: 15 });
    expect(at(1, 4)).not.toContainEqual({ line: 1, column: 27 });
    expect(at(2, 4)).toEqual([{ line: 2, column: 4 }]);
  });
  it("counts every thing that can read in an attribute: a store read beside one call maps to neither (R25)", () => {
    const at = callStarts("<K run={() => st.v + holder.c()} />;\n<K run={() => p.run()} />;");
    expect(at(1, 4)).toEqual([{ line: 1, column: 4 }]);
    expect(at(2, 4)).toContainEqual({ line: 2, column: 15 });
  });
  // ADR 1117b: Chrome rc.21 frames a native coercion or `instanceof` hook's caller where the compiled program's source map has a position (measured on the js-* cells).
  it("takes a wrapped template's outer start, a coercing binary's start and an instanceof's left operand to the row", () => {
    const at = callStarts('void `${o}`;\nvoid ("a" + o);\nconst b = {} instanceof K;\nvoid (f() + o);');
    expect(at(1, 1)).toContainEqual({ line: 1, column: 9 });
    expect(at(2, 7)).toContainEqual({ line: 2, column: 13 });
    expect(at(3, 12)).toContainEqual({ line: 3, column: 11 });
    // A left operand that runs code frames itself: its frame is not the operator's.
    expect(at(4, 7)).not.toContainEqual({ line: 4, column: 13 });
  });
  // ADR 1117c: the 1117b ranges stand for a row only where nothing else in them can frame there.
  describe("the 1117b ranges only where nothing else frames there (ADR 1117c)", () => {
    const head = "const o = {}; const p = {}; const jv: unknown = {}; const a = { b: 1 }; const K = {} as any;\n";
    const lines = (...body) => callStarts(head + body.join("\n") + "\n");
    it("keeps 1117b's cells: an inert left operand's frame stands for the instanceof", () => {
      const at = lines("void ({} instanceof K);", "const b = {} instanceof K; void b;", "if (jv instanceof K) void 0;", "jv instanceof K;", "void ([0, jv] instanceof K);", "void ({ x: jv, o } instanceof K);",
        "function h(q: unknown) { return q instanceof K; }");
      expect(at(2, 9)).toContainEqual({ line: 2, column: 7 });
      expect(at(3, 12)).toContainEqual({ line: 3, column: 11 });
      expect(at(4, 5)).toContainEqual({ line: 4, column: 5 });
      expect(at(4, 8)).toContainEqual({ line: 4, column: 5 });
      expect(at(5, 2)).toContainEqual({ line: 5, column: 1 });
      expect(at(6, 12)).toContainEqual({ line: 6, column: 7 });
      expect(at(7, 20)).toContainEqual({ line: 7, column: 7 });
      expect(at(8, 34)).toContainEqual({ line: 8, column: 33 });
    });
    it("gives no instanceof range to a left operand holding a member read, a call, a global or a spread", () => {
      const at = lines("void ([a.b] instanceof K);", "void ({ x: a.b } instanceof K);", "void ((a.b || {}) instanceof K);", "void ([g] instanceof K);", "void ({ a: g } instanceof K);",
        "void ([0, a.b] instanceof K);", "void ([f()] instanceof K);", "void ([...xs] instanceof K);", "void ({ [jv as string]: 1 } instanceof K);");
      const binary = { column: 7 };
      for (const [line, columns] of [[2, [7, 8, 9, 10, 11, 13]], [3, [8, 12, 13, 14, 16]], [4, [8, 9, 10, 12, 15]], [5, [8, 9]], [6, [9, 12]], [7, [8, 11, 13]], [8, [8, 9]], [9, [8, 11]], [10, [8, 10]]]) {
        for (const column of columns) expect(at(line, column)).not.toContainEqual({ line, ...binary });
      }
      // The member read keeps its own row: the access's start.
      expect(at(2, 10)).toContainEqual({ line: 2, column: 8 });
      expect(at(3, 14)).toContainEqual({ line: 3, column: 12 });
    });
    it("takes a coercing binary's start to its right operand only after a primitive literal", () => {
      const at = lines('void ("a" + o);', 'f("a" + o);', 'void ("a" + g);', "void (g + o);", "void (p + o);", "void (p + o + g);", "void ({} + o);", "void `${o}` + g;");
      expect(at(2, 7)).toContainEqual({ line: 2, column: 13 });
      expect(at(3, 3)).toContainEqual({ line: 3, column: 9 });
      expect(at(4, 7)).toContainEqual({ line: 4, column: 13 });
      // The left operand's own frame is at the same start: a global's getter, an object's coercion, an inner binary's.
      expect(at(5, 7)).toEqual([{ line: 5, column: 7 }]);
      expect(at(6, 7)).toEqual([{ line: 6, column: 7 }]);
      expect(at(7, 7)).toEqual([{ line: 7, column: 7 }]);
      expect(at(8, 7)).toEqual([{ line: 8, column: 7 }]);
      expect(at(9, 1)).toEqual([{ line: 9, column: 1 }]);
    });
    it("takes a template's wrapper to its substitutions only where the wrapper starts no larger expression", () => {
      const at = lines("void `${o}`;", "void typeof `${o}`;", "void `${o}` + g;", "void `${o}`, g;", "const s = void `${o}`;");
      expect(at(2, 1)).toContainEqual({ line: 2, column: 9 });
      expect(at(3, 1)).toContainEqual({ line: 3, column: 16 });
      expect(at(4, 1)).toEqual([{ line: 4, column: 1 }]);
      expect(at(5, 1)).toEqual([{ line: 5, column: 1 }]);
      expect(at(6, 11)).toContainEqual({ line: 6, column: 19 });
      // The template's own start still stands for its substitution, as on main.
      expect(at(4, 6)).toContainEqual({ line: 4, column: 9 });
    });
  });
  it("takes a frame at `in` to its right operand, and a tagged call's template to its tag", () => {
    const at = callStarts('v = "v" in s1;\nhf`x`;');
    expect(at(1, 5)).toContainEqual({ line: 1, column: 12 });
    expect(at(2, 3)).toContainEqual({ line: 2, column: 1 });
  });
  it("keeps the arguments of a Solid function that reads what it is handed", () => {
    const frame = frameStarts('import { deep, untrack } from "solid-js";\ndeep(st);\nuntrack(c);');
    expect(frame({ line: 2, column: 1 }, 0)).toContainEqual(expect.objectContaining({ line: 2, column: 6, handed: true }));
    expect(frame({ line: 3, column: 1 }, 0)).toEqual(expect.not.arrayContaining([expect.objectContaining({ handed: true })]));
  });
  // ADR 0981 (a judge position correction): Chrome (rc.21) puts the frame of a native or dependency read of what a `new`, a component element or `createComponent`
  // was handed on the statement's first token, not in the callee, so the value handed there is the withholding's place, as a call's argument is.
  it("hands a `new`'s arguments to a frame on its keyword, as to one on its callee", () => {
    const frame = frameStarts('import { Pkg } from "fz-pkg";\nimport { Local } from "./local";\nnew Pkg(r1, r2);\nnew Local(r1);');
    for (const column of [1, 5]) expect(frame({ line: 3, column }, 0)).toEqual(expect.arrayContaining([
      expect.objectContaining({ line: 3, column: 9, handed: true, index: 0, noTarget: true }), expect.objectContaining({ line: 3, column: 13, handed: true, index: 1 })]));
    // A frame deeper in the stack is not the constructor's read, and a project class is open (its body is authored).
    expect(frame({ line: 3, column: 1 }, 1)).toEqual(expect.not.arrayContaining([expect.objectContaining({ handed: true })]));
    expect(frame({ line: 4, column: 1 }, 0)).toContainEqual(expect.objectContaining({ line: 4, column: 11, handed: true, noTarget: false }));
    // The keyword is also where an enclosing call's callee starts (`new Function(s)(a)`): that call's argument stays handed, beside the constructor's.
    const nested = frameStarts('const x = new Function("g", "return g()")(a);');
    expect(nested({ line: 1, column: 11 }, 0)).toEqual(expect.arrayContaining([expect.objectContaining({ column: 43, handed: true }), expect.objectContaining({ column: 24, handed: true })]));
  });
  it("marks the arguments of a dependency's function, constructor, element and createComponent, and no project function's", () => {
    const frame = frameStarts('import { Pkg, run } from "fz-pkg";\nimport { Local } from "./local";\nimport { createComponent } from "solid-js";\nrun(a);\nLocal(a);\nnew Pkg(a);\n<Pkg v={a} />;\ncreateComponent(Pkg, { a });');
    const marked = (line) => frame({ line, column: 1 }, 0).filter((place) => place.handed).map((place) => place.dependency === true);
    expect(marked(4)).toEqual([true]);
    expect(marked(5)).toEqual([false]);
    expect(marked(6)).toEqual([true]);
    expect(marked(7)).toEqual([true]);
    expect(marked(8)).toEqual([true, true, true]);
  });
  it("hands a dependency element's attribute and spread values to a frame on its `<`, and no other element's", () => {
    const frame = frameStarts('import { Pkg } from "fz-pkg";\nimport { Show } from "solid-js";\nimport { Local } from "./local";\n<Pkg v={r1} {...rest} w="s" n={1} />;\n<Show when={r1}>x</Show>;\n<Local v={r1} />;\n<div class={r1} />;');
    const handed = (line) => frame({ line, column: 1 }, 0).filter((place) => place.handed);
    // The expressions of `v` and the spread, not the literal `w="s"`'s string (`n={1}` is an expression: its literal is handed like any argument).
    expect(handed(4).map((place) => place.column)).toEqual([9, 17, 32]);
    expect(frame({ line: 4, column: 3 }, 0)).toContainEqual(expect.objectContaining({ line: 4, column: 9, handed: true }));
    expect(frame({ line: 4, column: 1 }, 1).filter((place) => place.handed)).toEqual([]);
    // A Solid component, a project component and an intrinsic element have an authored frame (or none that reads native code): nothing is handed.
    for (const line of [5, 6, 7]) expect(handed(line), String(line)).toEqual([]);
  });
  it("hands what a dependency's call was handed to a frame on a member of its result, and what Reflect.apply forwards to a dependency's function", () => {
    const frame = frameStarts('import { make1, run } from "fz-pkg";\nimport { own } from "./own";\nmake1(a).value;\nmake1(a)[k];\nown(a).value;\nReflect.apply(run, undefined, [s]);\nReflect.apply(own, undefined, [s]);');
    const handed = (line, column) => frame({ line, column }, 0).filter((place) => place.handed);
    expect(handed(3, 8)).toEqual([expect.objectContaining({ line: 3, column: 7, index: 0, dependency: true })]);
    expect(handed(4, 8)).toEqual([expect.objectContaining({ line: 4, column: 7, index: 0, dependency: true })]);
    // A project function's result, a frame on the property itself and a frame deeper in the stack hand nothing.
    expect(handed(5, 6)).toEqual([]);
    expect(handed(3, 10)).toEqual([]);
    expect(frame({ line: 3, column: 8 }, 1).filter((place) => place.handed)).toEqual([]);
    // `Reflect.apply(run, …)` forwards to a dependency, `Reflect.apply(own, …)` to a project function (a store write's target is then its first argument only).
    expect(handed(6, 1).filter((place) => place.dependency === true).map((place) => place.column)).toEqual([15, 20, 31, 32]);
    expect(handed(7, 1).filter((place) => place.dependency === true)).toEqual([]);
  });
  // ADR 0981: a project helper that calls one of its own parameters (`f(...args)`) runs what the caller handed it, and the read's top frame is in the helper while the
  // call of the helper is a deeper frame: the arguments of that call are what was forwarded, but only to a deeper frame whose nearer frame is in such a call.
  it("hands the arguments of a project helper's call to the deeper frame, when the nearer frame is its call of its own parameter", () => {
    const source = 'import { site1 } from "fz-pkg";\nfunction apply1(f, ...args) { return f(...args); }\nfunction keep(f) { return 1; }\nuntrack(() => { apply1(site1, setW1); });\nkeep(site1, setW1);\nfunction twice(f) { f(); return 2; }\ntwice(site1, setW1);';
    const frame = frameStarts(source);
    const handed = (at, depth, inner) => frame(at, depth, inner).filter((place) => place.handed).map((place) => [place.line, place.column]);
    // The helper's `f(...args)` starts at line 2 column 38 (its `return` at 31); the call `apply1(` is line 4 column 17; `site1` is column 24 and `setW1` column 31.
    expect(handed({ line: 4, column: 17 }, 1, { line: 2, column: 38 })).toEqual([[4, 24], [4, 31]]);
    // The nearer frame is the helper's, not in a call of its parameter (`return` of a helper that calls nothing of its own): nothing is handed.
    expect(handed({ line: 4, column: 17 }, 1, { line: 2, column: 31 })).toEqual([]);
    // A helper that does not call its parameter (`keep`), or one that does where the nearer frame is not in that call (`twice`'s `return`): nothing; in the call, its arguments.
    expect(handed({ line: 5, column: 1 }, 1, { line: 3, column: 20 })).toEqual([]);
    expect(handed({ line: 7, column: 1 }, 1, { line: 6, column: 26 })).toEqual([]);
    expect(handed({ line: 7, column: 1 }, 1, { line: 6, column: 21 })).toEqual([[7, 7], [7, 14]]);
    // No nearer frame, a nearer frame of another file, and the top frame (the call's own callee) keep the rules they had.
    expect(handed({ line: 4, column: 17 }, 1, undefined)).toEqual([]);
    expect(handed({ line: 4, column: 17 }, 1, { line: 2, column: 38, file: "other.ts" })).toEqual([]);
    expect(handed({ line: 4, column: 17 }, 0)).toEqual([[4, 24], [4, 31]]);
  });
  it("hands the attribute values of a tag a project binding holds as dynamic(...) or lazy(...), like a dependency's element, and not another binding's", () => {
    const frame = frameStarts('import { dynamic } from "@solidjs/web";\nimport { lazy } from "solid-js";\nimport { Site1 } from "fz-pkg";\nconst Dyn = dynamic(() => Site1);\nconst Lz = lazy(() => import("fz-pkg"));\nconst Plain = () => null;\n<Dyn v={a} />;\n<Lz v={a} />;\n<Plain v={a} />;');
    const handed = (line) => frame({ line, column: 1 }, 0).filter((place) => place.handed).map((place) => place.dependency === true);
    expect(handed(7)).toEqual([true]);
    expect(handed(8)).toEqual([true]);
    expect(handed(9)).toEqual([]);
  });
  it("marks a handed bare identifier, and the identifiers a props object or an array holds, for a dependency's call", () => {
    const frame = frameStarts('import { run, writeHeld } from "fz-pkg";\nimport { createComponent } from "solid-js";\nrun(s1, holder.c, (s2));\ncreateComponent(run, { s1, a: s2, b: holder.c });\nReflect.apply(run, undefined, [s1, holder.c]);');
    const bare = (line) => frame({ line, column: 1 }, 0).filter((place) => place.handed).map((place) => [place.column, place.bare === true]);
    expect(bare(3)).toEqual([[5, true], [9, false], [19, true]]);
    expect(bare(4)).toEqual([[17, true], [22, false], [24, true], [31, true]]);
    expect(bare(5)).toEqual([[15, true], [20, true], [31, false], [32, true]]);
  });
  it("hands createComponent's arguments when it renders a dependency's component, and not for Solid's, a project's or an unknown component", () => {
    const frame = frameStarts('import { createComponent } from "solid-js";\nimport { Pkg } from "fz-pkg";\nimport { Local } from "./local";\ncreateComponent(Pkg, { r1 });\ncreateComponent(Local, { r1 });\ncreateComponent(Unknown, { r1 });\ncreateComponent(Pkg);');
    expect(frame({ line: 4, column: 1 }, 0).filter((place) => place.handed)).toEqual([
      expect.objectContaining({ line: 4, column: 17, index: 0, noTarget: true }), expect.objectContaining({ line: 4, column: 22, index: 1, noTarget: true }),
      expect.objectContaining({ line: 4, column: 24, index: 2, noTarget: true, bare: true })]);
    for (const line of [5, 6, 7]) expect(frame({ line, column: 1 }, 0).filter((place) => place.handed), String(line)).toEqual([]);
    expect(frame({ line: 4, column: 1 }, 1).filter((place) => place.handed)).toEqual([]);
  });
  it("gives an element's frame the whole pattern, never another element", () => {
    const at = callStarts("const { a, b } = st;\nconst [{ c }, d] = xs;\n({ e, f } = st);");
    expect(at(1, 12)).toEqual([{ line: 1, column: 12 }, { line: 1, column: 7 }]);
    expect(at(2, 10)).toEqual([{ line: 2, column: 10 }, { line: 2, column: 7 }]);
    expect(at(3, 7)).toEqual([{ line: 3, column: 7 }, { line: 3, column: 2 }]);
  });
  it("finds the statement of a position on a line of several", () => {
    const at = statementAt("const a = f(st); const b = g(c); log(a);", 1, 26);
    expect(at.contains(1, 28)).toBe(true);
    expect(at.contains(1, 13)).toBe(false);
    const iife = statementAt("(() => { const t = f(st); t.a = 7; })();\nx();", 1, 30);
    expect(iife.contains(1, 22)).toBe(true);
  });
});
