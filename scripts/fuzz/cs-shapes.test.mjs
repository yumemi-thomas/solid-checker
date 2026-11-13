import { describe, expect, it } from "vitest";
import { buildCase } from "./emit.mjs";
import { generateProgram } from "./generate.mjs";
import { recallLegalShapes } from "./legal.mjs";
import { BASE_ONLY_CONTEXTS, SPEC } from "./misuse.mjs";
import { censusCells, recallShapes } from "./matrix.mjs";
import { isValid } from "./model.mjs";
import { CS_CODES, CS_CONTEXTS, CS_CONTEXT_NAMES, CS_FORM_NAMES, CS_HOSTS, CS_RECOGNIZED, CS_ROUTE_NAMES, csParts, csPlacement, csShape } from "./cs-shapes.mjs";

// Batch 11, slice 9B (ADR 0973): the computation-start shapes, an async function declaration the program starts as a tracked async computation through a form the producers'
// index of computations does not follow (a project wrapper's parameter, `.call`, `Reflect.apply`, an array element), as generator cells with their twins (nothing starts it, the read
// ahead of the await), and the routes the review of #233 found silent (B1 to B3): how the continuation reaches the read.

const AWAIT = "UNTRACKED_READ_AFTER_AWAIT";
const shapesOf = (context) => (CS_CONTEXTS[context] === "misuse" ? recallShapes : recallLegalShapes)(AWAIT, context);
const textOf = (context) => {
  const shape = shapesOf(context).find((one) => one.binding === "direct" && one.layer === "none");
  const program = generateProgram(7, { profile: "misuse", misusePlan: { code: AWAIT, context, variant: shape.variant, binding: "direct", layer: "none", legal: CS_CONTEXTS[context] === "legal" } });
  return { program, built: buildCase(program, { id: `cs-${context}` }) };
};
const run = (context) => {
  const { program, built } = textOf(context);
  expect(isValid(program), context).toBe(true);
  expect(built.sites, context).toHaveLength(1);
  return built.text.split("\n").map((line) => line.trim());
};

describe("the cs contexts", () => {
  it("are the hidden starts on two hosts and two routes, and the review's routes over one start, each with its two twins, each name parsed back", () => {
    expect(CS_FORM_NAMES).toEqual(["wrapper", "call", "reflect", "element"]);
    expect(CS_HOSTS).toEqual(["memo", "effect"]);
    expect(CS_ROUTE_NAMES).toEqual(["getter", "ns-method", "ns-arrow", "ns-computed", "ns-optional", "dotcall", "apply", "reflect", "iife", "iife-fn", "iife-async", "gen", "proxy-ref", "defprop-ref"]);
    // 4 starts x 2 routes (none, getter), 10 routes over the wrapper (B2, B3), 3 routes over the recognized start (B1), each on two hosts, each with its two twins
    expect(CS_CONTEXT_NAMES).toHaveLength((CS_FORM_NAMES.length * 2 + 10 + 3) * CS_HOSTS.length * 3);
    for (const name of CS_CONTEXT_NAMES) {
      const parts = csParts(name);
      expect(parts, name).not.toBeNull();
      expect(CS_CONTEXTS[name]).toBe(parts.never || parts.before ? "legal" : "misuse");
      expect(name).toBe(`cs-${parts.form}-${parts.host}${parts.route ? `-${parts.route}` : ""}${parts.never ? "-never" : ""}${parts.before ? "-before" : ""}`);
    }
    // an unknown form, host or route, a twin suffix twice, the route after a twin suffix, a route over a start it is not crossed with, another axis's name
    for (const other of ["cs-getter-memo", "cs-call-apply", "cs-call-memo-never-before", "cs-call-memo-after", "cs-call-memo-never-getter", "cs-wrapper-memo-nothing", "cs-call-memo-iife", "cs-rec-memo-getter", "cs-wrapper-memo-gen", "cs-reflect-memo-reflect", "sf-foreach-memo-async", "memo-async"]) expect(csParts(other), other).toBeNull();
    // `reflect` is both a start and a route: the position says which
    expect(csParts("cs-wrapper-memo-reflect")).toMatchObject({ form: "wrapper", route: "reflect" });
    expect(csParts("cs-reflect-memo")).toMatchObject({ form: "reflect", route: "" });
    expect(csParts("cs-rec-effect-proxy-ref-never")).toMatchObject({ form: CS_RECOGNIZED, host: "effect", route: "proxy-ref", never: true });
  });
  it("are placed for SC1002 and for no other code", () => {
    const placement = csPlacement();
    expect(Object.keys(placement)).toEqual(CS_CODES);
    expect(CS_CODES).toEqual([AWAIT]);
    expect(placement[AWAIT].misuse).toHaveLength(CS_CONTEXT_NAMES.length / 3);
    expect(placement[AWAIT].legal).toHaveLength((CS_CONTEXT_NAMES.length / 3) * 2);
  });
  it("are cells of SPEC on their side, one base shape, and not the census's", () => {
    for (const context of CS_CONTEXT_NAMES) {
      const side = CS_CONTEXTS[context] === "misuse" ? SPEC[AWAIT].recallContexts : SPEC[AWAIT].recallLegal;
      expect(side, context).toContain(context);
      expect(shapesOf(context), context).toHaveLength(1);
      expect(BASE_ONLY_CONTEXTS()).toContain(context);
    }
    const [shape] = shapesOf("cs-wrapper-memo");
    expect(`${shape.binding}/${shape.layer}`).toBe("direct/none");
    expect(csShape("cs-wrapper-memo", { binding: "alias", layer: "none" })).toBe(false);
    expect(csShape("cs-wrapper-memo", { binding: "direct", layer: "iife" })).toBe(false);
    expect(csShape("memo-async", { binding: "alias", layer: "iife" })).toBe(true);
    expect(censusCells().filter(({ context }) => CS_CONTEXT_NAMES.includes(context))).toEqual([]);
  });
});

describe("the cs programs", () => {
  it("declare the function, read behind its await, and start it where the form says", () => {
    const primitive = { memo: "createMemo", effect: "createEffect" };
    const type = { memo: "(compute: () => Promise<number>) => unknown", effect: "(compute: () => Promise<number>, effect: () => void) => unknown" };
    const rest = { memo: "", effect: ", () => {}" };
    for (const host of CS_HOSTS) {
      for (const [form, start] of [
        ["wrapper", "track1(work1);"],
        ["call", `(${primitive[host]} as ${type[host]}).call(undefined, work1${rest[host]});`],
        ["reflect", `Reflect.apply(${primitive[host]} as ${type[host]}, undefined, [work1${rest[host]}]);`],
        ["element", `primitives1[0](work1${rest[host]});`],
      ]) {
        const lines = run(`cs-${form}-${host}`);
        expect(lines, `${form}/${host}`).toContain(start);
        // the started function is a function DECLARATION at module scope, the read stands behind its await
        const declared = lines.indexOf("async function work1() {");
        expect(declared, `${form}/${host}`).toBeGreaterThanOrEqual(0);
        expect(lines.indexOf("await Promise.resolve();"), `${form}/${host}`).toBe(declared + 1);
        expect(lines[declared + 2], `${form}/${host}`).toMatch(/^r1\(\);$/);
        // the start is written in the component, after the declaration
        expect(lines.indexOf(start), `${form}/${host}`).toBeGreaterThan(declared);
        // the primitive is imported from solid-js
        expect(lines[0], `${form}/${host}`).toContain(primitive[host]);
      }
    }
    // the wrapper form hands the parameter on, in the wrapper's own body
    expect(run("cs-wrapper-memo")).toContain("function track1(compute: () => Promise<number>) { return createMemo(compute); };");
    expect(run("cs-wrapper-effect")).toContain("function track1(compute: () => Promise<number>) { createEffect(compute, () => {}); };");
    expect(run("cs-element-memo")).toContain("const primitives1 = [createMemo as (compute: () => Promise<number>) => unknown];");
  });
  it("keep the twin that starts nothing: the names are used and no computation is made", () => {
    for (const form of CS_FORM_NAMES) for (const host of CS_HOSTS) {
      const live = run(`cs-${form}-${host}`);
      const never = run(`cs-${form}-${host}-never`);
      expect(never, `${form}/${host}`).toContain("void work1;");
      // what the twin leaves out is the start, and nothing it keeps is a call of the primitive on the function
      const started = live.filter((line) => !never.includes(line));
      expect(started.length, `${form}/${host}`).toBeGreaterThan(0);
      expect(never.some((line) => /\bwork1\b/.test(line) && /(\.call|Reflect\.apply|\[0\]|track1)\(/.test(line)), `${form}/${host}`).toBe(false);
    }
  });
  it("keep the twin that reads before the await, where the read is a dependency", () => {
    for (const form of CS_FORM_NAMES) for (const host of CS_HOSTS) {
      const before = run(`cs-${form}-${host}-before`);
      const live = run(`cs-${form}-${host}`);
      const declared = before.indexOf("async function work1() {");
      expect(before[declared + 1], `${form}/${host}`).toBe("r1();");
      expect(before[declared + 2], `${form}/${host}`).toBe("await Promise.resolve();");
      // the same start as the program, nothing else moves
      expect(before.filter((line) => !live.includes(line)), `${form}/${host}`).toEqual([]);
    }
  });
  it("write the read in a getter the started function reaches through `peek()`, behind its await (ahead of it in the twin)", () => {
    for (const form of CS_FORM_NAMES) for (const host of CS_HOSTS) {
      const lines = run(`cs-${form}-${host}-getter`);
      const awaitAt = lines.indexOf("await Promise.resolve();");
      // the site is written in the getter of a module-level object, which a project function reads
      expect(lines.some((line) => /^const holder1 = \{ get value\(\) \{$/.test(line)), `${form}/${host}`).toBe(true);
      expect(lines, `${form}/${host}`).toContain("function peek1() { return holder1.value; };");
      expect(lines[lines.indexOf("const holder1 = { get value() {") + 1], `${form}/${host}`).toBe("r1();");
      // the started function calls it after its await, the twin before it, and the `-never` twin starts nothing
      const call = lines.indexOf("void peek1();");
      expect(call, `${form}/${host}`).toBeGreaterThan(awaitAt);
      const before = run(`cs-${form}-${host}-getter-before`);
      expect(before.indexOf("void peek1();"), `${form}/${host}`).toBeLessThan(before.indexOf("await Promise.resolve();"));
      expect(run(`cs-${form}-${host}-getter-never`), `${form}/${host}`).toContain("void work1;");
    }
  });
  it("write the call of each route after the `await` of a hidden start, the site in the helper it reaches (B2, B3)", () => {
    // [route, the helper's first line, the call the started function writes]
    for (const [route, helper, calls] of [
      ["ns-method", "const ns1 = { run() {", "ns1.run();"],
      ["ns-arrow", "const ns1 = { run: () => {", "ns1.run();"],
      ["ns-computed", "const ns1 = { run() {", 'ns1["run"]();'],
      ["ns-optional", "const ns1 = { run() {", "ns1?.run();"],
      ["dotcall", "function reader1() {", "reader1.call(undefined);"],
      ["apply", "function reader1() {", "reader1.apply(undefined, []);"],
      ["reflect", "function reader1() {", "Reflect.apply(reader1, undefined, []);"],
    ]) {
      for (const host of CS_HOSTS) {
        const lines = run(`cs-wrapper-${host}-${route}`);
        const at = lines.indexOf(helper);
        expect(at, `${route}/${host}`).toBeGreaterThanOrEqual(0);
        expect(lines[at + 1], `${route}/${host}`).toBe("r1();");
        // module scope, ahead of the started function, which writes the call after its `await`
        const declared = lines.indexOf("async function work1() {");
        expect(at, `${route}/${host}`).toBeLessThan(declared);
        expect(lines[declared + 1], `${route}/${host}`).toBe("await Promise.resolve();");
        expect(lines[declared + 2], `${route}/${host}`).toBe(calls);
        // the start is the hidden one: a wrapper's parameter
        expect(lines, `${route}/${host}`).toContain("track1(work1);");
      }
    }
  });
  it("write a function literal called where it is written, the site in it, behind the `await` (B3)", () => {
    for (const [route, open, close] of [["iife", "(() => {", "})();"], ["iife-fn", "(function () {", "})();"], ["iife-async", "await (async () => {", "})();"]]) {
      for (const host of CS_HOSTS) {
        const lines = run(`cs-wrapper-${host}-${route}`);
        const declared = lines.indexOf("async function work1() {");
        expect(lines[declared + 1], `${route}/${host}`).toBe("await Promise.resolve();");
        expect(lines[declared + 2], `${route}/${host}`).toBe(open);
        expect(lines[declared + 3], `${route}/${host}`).toBe("r1();");
        expect(lines[declared + 4], `${route}/${host}`).toBe(close);
        // the twin reads ahead of the await
        const before = run(`cs-wrapper-${host}-${route}-before`);
        const at = before.indexOf("async function work1() {");
        expect(before[at + 1], `${route}/${host}`).toBe(open);
        expect(before[at + 4], `${route}/${host}`).toBe("await Promise.resolve();");
      }
    }
  });
  it("write the three routes a closure's list of reads does not show, in a recognized computation (B1)", () => {
    for (const host of CS_HOSTS) {
      const primitive = host === "memo" ? "createMemo" : "createEffect";
      for (const [route, lines] of [
        ["gen", ["function* numbers1() {", "r1();", "yield 1;", "const stored1 = numbers1();", "function peek1() { return [...stored1]; };"]],
        ["proxy-ref", ["function reader1() {", "r1();", "return 1;", "const traps1 = { get: reader1 };", "const proxy1: any = new Proxy({}, traps1);", "function peek1() { return proxy1.x; };"]],
        ["defprop-ref", ["function reader1() {", "r1();", "return 1;", "const desc1 = { get: reader1 };", "const installed1: any = {};", 'Object.defineProperty(installed1, "value", desc1);', "function peek1() { return installed1.value; };"]],
      ]) {
        const text = run(`cs-${CS_RECOGNIZED}-${host}-${route}`);
        for (const line of lines) expect(text, `${route}/${host}`).toContain(line);
        // the compute is written inline in the primitive (a start the index recognizes), and calls `peek1()` after its `await`
        const start = text.findIndex((line) => line.includes(`${primitive}(async () => {`));
        expect(start, `${route}/${host}`).toBeGreaterThanOrEqual(0);
        expect(text[start + 1], `${route}/${host}`).toBe("await Promise.resolve();");
        expect(text[start + 2], `${route}/${host}`).toBe("void peek1();");
        // no wrapper, no hidden start
        expect(text.some((line) => line.startsWith("function track1")), `${route}/${host}`).toBe(false);
        // the `-never` twin declares the same function and starts nothing; the `-before` twin calls ahead of the `await`
        const never = run(`cs-${CS_RECOGNIZED}-${host}-${route}-never`);
        expect(never, `${route}/${host}`).toContain("void work1;");
        expect(never.some((line) => line.includes(`${primitive}(async`)), `${route}/${host}`).toBe(false);
        const before = run(`cs-${CS_RECOGNIZED}-${host}-${route}-before`);
        const at = before.findIndex((line) => line.includes(`${primitive}(async () => {`));
        expect(before[at + 1], `${route}/${host}`).toBe("void peek1();");
        expect(before[at + 2], `${route}/${host}`).toBe("await Promise.resolve();");
      }
    }
  });
  it("every cell builds a neutral, valid program of one site", () => {
    for (const context of CS_CONTEXT_NAMES) {
      const { program, built } = textOf(context);
      expect(isValid(program), context).toBe(true);
      expect(built.neutral, context).toBe(true);
      expect(built.sites, context).toHaveLength(1);
    }
  });
});
