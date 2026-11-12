import { describe, expect, it } from "vitest";
import { buildCase } from "./emit.mjs";
import { generateProgram } from "./generate.mjs";
import { recallLegalShapes } from "./legal.mjs";
import { BASE_ONLY_CONTEXTS, SPEC } from "./misuse.mjs";
import { censusCells, recallShapes } from "./matrix.mjs";
import { isValid } from "./model.mjs";
import { RM_CODES, RM_CONTEXTS, RM_CONTEXT_NAMES, RM_FORM_NAMES, RM_HOSTS, RS_CONTEXTS, RS_CONTEXT_NAMES, RS_FORM_NAMES, RS_HOSTS, rmParts, rsParts, rvPlacement, rvShape } from "./rv-shapes.mjs";

// Batch 10c: the spellings the Fable review of batch 10b found silent (`rs-*`: a stored function invoked through a member slot, `.call`, `.apply`, a reassigned `let`, a tag or `new`; `rm-*`: a
// member route written as a function value), as generator cells with their twins.

const AWAIT = "UNTRACKED_READ_AFTER_AWAIT";
const SIDES = { ...RS_CONTEXTS, ...RM_CONTEXTS };
const codeOf = (context) => (context.startsWith("rs-") ? AWAIT : Object.entries(RM_CODES).find(([, hosts]) => hosts.includes(rmParts(context).host))[0]);
const shapesOf = (context) => (SIDES[context] === "misuse" ? recallShapes : recallLegalShapes)(codeOf(context), context);
const textOf = (context, code = codeOf(context)) => {
  const shape = (SIDES[context] === "misuse" ? recallShapes : recallLegalShapes)(code, context).find((one) => one.binding === "direct" && one.layer === "none");
  const program = generateProgram(7, { profile: "misuse", misusePlan: { code, context, variant: shape.variant, binding: "direct", layer: "none", legal: SIDES[context] === "legal" } });
  return { program, built: buildCase(program, { id: `rv-${context}` }) };
};
const run = (context, code) => {
  const { program, built } = textOf(context, code);
  expect(isValid(program), context).toBe(true);
  expect(built.sites, context).toHaveLength(1);
  return built.text.split("\n").map((line) => line.trim());
};

describe("the rs contexts (stored-function call forms)", () => {
  it("are fourteen forms on two hosts, each with its two twins, each name parsed back", () => {
    expect(RS_FORM_NAMES).toEqual(["prop", "optprop", "call", "apply", "let", "tag", "tagidx", "new", "propcall", "iface", "bind", "field", "propmeth", "thismeth"]);
    expect(RS_HOSTS).toEqual(["memo-async", "effect-async"]);
    expect(RS_CONTEXT_NAMES).toHaveLength(RS_FORM_NAMES.length * RS_HOSTS.length * 3);
    for (const name of RS_CONTEXT_NAMES) {
      const parts = rsParts(name);
      expect(parts, name).not.toBeNull();
      expect(RS_CONTEXTS[name]).toBe(parts.never || parts.before ? "legal" : "misuse");
      expect(name).toBe(`rs-${parts.form}-${parts.host}${parts.never ? "-never" : ""}${parts.before ? "-before" : ""}`);
    }
    for (const other of ["rs-getter-memo-async", "rs-prop-settled", "rs-prop-memo-async-never-before", "sf-prop-memo-async", "ob-prop-memo-async", "rm-prop-memo-async"]) expect(rsParts(other), other).toBeNull();
  });
  it("invoke the stored function behind the await through their form, and keep each twin's names used", () => {
    for (const [form, invocation] of [["prop", "bus1.cb();"], ["optprop", "hold1.cb?.();"], ["call", "fn1.call(undefined);"], ["apply", "cbs1[0].apply(undefined, []);"], ["let", "fn1();"], ["tag", "tag1`x`;"], ["tagidx", "tags1[0]`x`;"], ["new", "new K1();"], ["propcall", "bus1.cb.call(undefined);"], ["iface", "svc1.run();"], ["bind", "bound1();"], ["field", "holder1.cb();"], ["propmeth", "emitter1.emit();"], ["thismeth", "emitter1.emit();"]]) {
      for (const host of RS_HOSTS) {
        const lines = run(`rs-${form}-${host}`);
        expect(lines, `${form}/${host}`).toContain(invocation);
        expect(lines.some((line) => /^function hx1\(\) \{$/.test(line)), `${form}/${host}`).toBe(true);
        expect(lines.indexOf(invocation), `${form}/${host}`).toBeGreaterThan(lines.indexOf("await Promise.resolve();"));
        const never = run(`rs-${form}-${host}-never`);
        expect(never, `${form}/${host} never`).not.toContain(invocation);
        expect(never.some((line) => /^void [A-Za-z]\w*;$/.test(line)), `${form}/${host} never`).toBe(true);
        const before = run(`rs-${form}-${host}-before`);
        expect(before.indexOf(invocation), `${form}/${host} before`).toBeGreaterThanOrEqual(0);
        expect(before.indexOf(invocation), `${form}/${host} before`).toBeLessThan(before.indexOf("await Promise.resolve();"));
      }
    }
    expect(run("rs-let-memo-async")).toContain("let fn1: () => unknown = () => {};");
    expect(run("rs-new-memo-async")).toContain("ctors1.push(hx1 as unknown as new () => object);");
  });
});

describe("the rm contexts (member routes written as function values)", () => {
  it("are fourteen forms, the thenable on the async hosts only, each with its twin, in the host's callback (rm) and at module scope (rh), each name parsed back", () => {
    expect(RM_FORM_NAMES).toHaveLength(14);
    expect(RM_HOSTS).toEqual(["memo-async", "effect-async", "settled", "tracked"]);
    expect(RM_CONTEXT_NAMES).toHaveLength((14 * 2 * 2 + 13 * 2 * 2) * 2);
    for (const name of RM_CONTEXT_NAMES) {
      const parts = rmParts(name);
      expect(parts, name).not.toBeNull();
      expect(RM_CONTEXTS[name]).toBe(parts.never ? "legal" : "misuse");
      expect(name).toBe(`r${parts.hoisted ? "h" : "m"}-${parts.form}-${parts.host}${parts.never ? "-never" : ""}`);
    }
    for (const other of ["rm-then-arrow-settled", "rh-then-arrow-tracked", "rm-getter-memo-async", "rh-fnname-apply", "rm-fnname-memo-async-before", "ob-fnname-memo-async", "rx-fnname-memo-async"]) expect(rmParts(other), other).toBeNull();
  });
  it("are placed for SC1002's async computes and SC3001's three codes on the two leaves, and for no other code", () => {
    const placement = rvPlacement();
    expect(Object.keys(placement).sort()).toEqual(["CLEANUP_IN_FORBIDDEN_SCOPE", "FLUSH_IN_TRACKED_QUEUE_CALLBACK", "PRIMITIVE_IN_FORBIDDEN_SCOPE", AWAIT]);
    expect(placement[AWAIT].misuse).toHaveLength(RS_FORM_NAMES.length * 2 + 14 * 2 * 2);
    expect(placement[AWAIT].legal).toHaveLength(RS_FORM_NAMES.length * 2 * 2 + 14 * 2 * 2);
    for (const code of ["CLEANUP_IN_FORBIDDEN_SCOPE", "PRIMITIVE_IN_FORBIDDEN_SCOPE", "FLUSH_IN_TRACKED_QUEUE_CALLBACK"]) {
      expect(placement[code].misuse, code).toHaveLength(13 * 2 * 2);
      expect(placement[code].legal, code).toHaveLength(13 * 2 * 2);
    }
  });
  it("trigger the route from the host's callback and keep the twin's names used", () => {
    const text = (context, code) => run(context, code);
    expect(text("rm-tostring-arrow-memo-async")).toContain("const o1 = { toString: () => {");
    expect(text("rm-tostring-arrow-memo-async")).toContain("void `${o1}`;");
    expect(text("rm-tostring-arrow-memo-async-never")).toContain("void o1;");
    expect(text("rm-tostring-arrow-memo-async-never")).not.toContain("void `${o1}`;");
    expect(text("rm-valueof-fn-effect-async")).toContain("const o1 = { valueOf: function () {");
    expect(text("rm-proxy-get-arrow-memo-async")).toContain("const px1 = new Proxy({} as Record<string, number>, { get: (_t, _k) => {");
    expect(text("rm-iter-fn-memo-async")).toContain("const o1: Iterable<number> = { [Symbol.iterator]: function* () {");
    expect(text("rm-then-arrow-memo-async")).toContain("await th1;");
    expect(text("rm-fnname-effect-async")).toContain("const o1 = { toString: show1 };");
    expect(text("rm-assigned-memo-async")).toContain("o1.toString = () => {");
    expect(text("rm-proto-memo-async")).toContain("K1.prototype.toString = function () {");
    expect(text("rm-defprop-get-memo-async")).toContain('Object.defineProperty(o1, "x", { get: () => {');
    expect(text("rm-defprop-set-memo-async")).toContain("o1.x = 1;");
    // rh-*: the route at module scope, the host's callback only triggers it
    for (const [context, head, trigger] of [["rh-tostring-arrow-memo-async", "const o1 = { toString: () => {", "void `${o1}`;"], ["rh-assigned-effect-async", "o1.toString = () => {", "void `${o1}`;"], ["rh-defprop-get-memo-async", 'Object.defineProperty(o1, "x", { get: () => {', "void o1.x;"]]) {
      const lines = run(context);
      const component = lines.findIndex((line) => line.startsWith("export function C1"));
      expect(lines.findIndex((line) => line.includes(head)), context).toBeGreaterThanOrEqual(0);
      expect(lines.findIndex((line) => line.includes(head)), context).toBeLessThan(component);
      expect(lines.indexOf(trigger), context).toBeGreaterThan(component);
      const never = run(`${context}-never`);
      expect(never, `${context} never`).not.toContain(trigger);
      expect(never.findIndex((line) => line.includes(head)), `${context} never`).toBeGreaterThanOrEqual(0);
    }
    expect(text("rh-then-arrow-memo-async")).toContain("await th1;");
    // the leaf hosts: the same route inside `onSettled` and `createTrackedEffect`
    expect(text("rm-tostring-arrow-settled", "CLEANUP_IN_FORBIDDEN_SCOPE")).toContain("onSettled(() => {");
    expect(text("rm-assigned-tracked", "PRIMITIVE_IN_FORBIDDEN_SCOPE")).toContain("createTrackedEffect(() => {");
    expect(text("rh-proxy-get-arrow-settled", "CLEANUP_IN_FORBIDDEN_SCOPE")).toContain("onSettled(() => {");
  });
});

describe("the rv cells", () => {
  it("are cells of SPEC on their side, one base shape, and not the census's", () => {
    expect(rvShape("rs-prop-memo-async", { binding: "alias", layer: "none" })).toBe(false);
    expect(rvShape("rm-assigned-settled", { binding: "direct", layer: "iife" })).toBe(false);
    expect(rvShape("memo-async", { binding: "alias", layer: "iife" })).toBe(true);
    for (const context of RS_CONTEXT_NAMES) {
      const side = RS_CONTEXTS[context] === "misuse" ? SPEC[AWAIT].recallContexts : SPEC[AWAIT].recallLegal;
      expect(side, context).toContain(context);
      expect(shapesOf(context), context).toHaveLength(1);
      expect(BASE_ONLY_CONTEXTS()).toContain(context);
    }
    for (const context of RM_CONTEXT_NAMES) {
      const code = codeOf(context);
      const side = RM_CONTEXTS[context] === "misuse" ? SPEC[code].recallContexts : SPEC[code].recallLegal;
      expect(side, `${code}/${context}`).toContain(context);
      expect(shapesOf(context).length, context).toBe(1);
      expect(BASE_ONLY_CONTEXTS()).toContain(context);
    }
    expect(censusCells().filter(({ context }) => RS_CONTEXT_NAMES.includes(context) || RM_CONTEXT_NAMES.includes(context))).toEqual([]);
  });
  it("every cell builds a neutral, valid program of one site", () => {
    for (const context of [...RS_CONTEXT_NAMES, ...RM_CONTEXT_NAMES]) {
      const codes = context.startsWith("rs-") ? [AWAIT] : Object.entries(RM_CODES).filter(([, hosts]) => hosts.includes(rmParts(context).host)).map(([code]) => code);
      for (const code of codes) {
        const { program, built } = textOf(context, code);
        expect(isValid(program), `${code}/${context}`).toBe(true);
        expect(built.neutral, `${code}/${context}`).toBe(true);
        expect(built.sites, `${code}/${context}`).toHaveLength(1);
      }
    }
  });
});
