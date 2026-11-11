import { describe, expect, it } from "vitest";
import { buildCase } from "./emit.mjs";
import { generateProgram } from "./generate.mjs";
import { recallLegalShapes } from "./legal.mjs";
import { BASE_ONLY_CONTEXTS, SPEC } from "./misuse.mjs";
import { censusCells, recallShapes } from "./matrix.mjs";
import { isValid } from "./model.mjs";
import { SF_CODES, SF_CONTEXTS, SF_CONTEXT_NAMES, SF_FORM_NAMES, SF_HOSTS, sfParts, sfPlacement, sfShape } from "./sf-shapes.mjs";

// ADR 0964: the stored-function shapes, a function the program stored where nothing follows it and invoked through an unresolved call after the host's `await`, as generator cells
// with their twins (the function never invoked, invoked before the await).

const AWAIT = "UNTRACKED_READ_AFTER_AWAIT";
const shapesOf = (context) => (SF_CONTEXTS[context] === "misuse" ? recallShapes : recallLegalShapes)(AWAIT, context);
const textOf = (context) => {
  const shape = shapesOf(context).find((one) => one.binding === "direct" && one.layer === "none");
  const program = generateProgram(7, { profile: "misuse", misusePlan: { code: AWAIT, context, variant: shape.variant, binding: "direct", layer: "none", legal: SF_CONTEXTS[context] === "legal" } });
  return { program, built: buildCase(program, { id: `sf-${context}` }) };
};
const run = (context) => {
  const { program, built } = textOf(context);
  expect(isValid(program), context).toBe(true);
  expect(built.sites, context).toHaveLength(1);
  return (built.text.split("\n")).map((line) => line.trim());
};

describe("the sf contexts", () => {
  it("are seven forms on two hosts, each with its two twins, each name parsed back", () => {
    expect(SF_FORM_NAMES).toEqual(["foreach", "index", "const", "mapget", "bus", "promise", "param"]);
    expect(SF_HOSTS).toEqual(["memo-async", "effect-async"]);
    expect(SF_CONTEXT_NAMES).toHaveLength(SF_FORM_NAMES.length * SF_HOSTS.length * 3);
    for (const name of SF_CONTEXT_NAMES) {
      const parts = sfParts(name);
      expect(parts, name).not.toBeNull();
      expect(SF_CONTEXTS[name]).toBe(parts.never || parts.before ? "legal" : "misuse");
      expect(name).toBe(`sf-${parts.form}-${parts.host}${parts.never ? "-never" : ""}${parts.before ? "-before" : ""}`);
    }
    // an unknown form or host, a twin suffix twice, another axis's name
    for (const other of ["sf-getter-memo-async", "sf-foreach-apply", "sf-foreach-memo-async-never-before", "sf-foreach-memo-async-after", "ob-getter-memo-async", "memo-async"]) expect(sfParts(other), other).toBeNull();
  });
  it("are placed for SC1002's async computes and for no other code", () => {
    const placement = sfPlacement();
    expect(Object.keys(placement)).toEqual(SF_CODES);
    expect(SF_CODES).toEqual([AWAIT]);
    expect(placement[AWAIT].misuse).toHaveLength(SF_FORM_NAMES.length * SF_HOSTS.length);
    expect(placement[AWAIT].legal).toHaveLength(SF_FORM_NAMES.length * SF_HOSTS.length * 2);
    expect(placement[AWAIT].misuse.every((name) => !sfParts(name).never && !sfParts(name).before)).toBe(true);
  });
  it("are cells of SPEC on their side, one base shape, and not the census's", () => {
    for (const context of SF_CONTEXT_NAMES) {
      const side = SF_CONTEXTS[context] === "misuse" ? SPEC[AWAIT].recallContexts : SPEC[AWAIT].recallLegal;
      expect(side, context).toContain(context);
      expect(shapesOf(context), context).toHaveLength(1);
      expect(BASE_ONLY_CONTEXTS()).toContain(context);
    }
    const [shape] = shapesOf("sf-foreach-memo-async");
    expect(`${shape.binding}/${shape.layer}`).toBe("direct/none");
    expect(sfShape("sf-foreach-memo-async", { binding: "alias", layer: "none" })).toBe(false);
    expect(sfShape("sf-foreach-memo-async", { binding: "direct", layer: "iife" })).toBe(false);
    expect(sfShape("memo-async", { binding: "alias", layer: "iife" })).toBe(true);
    expect(censusCells().filter(({ context }) => SF_CONTEXT_NAMES.includes(context))).toEqual([]);
  });
});

describe("the sf programs", () => {
  it("store the function at module scope and invoke it from the host's callback after the await", () => {
    for (const [form, invocation] of [["foreach", "cbs1.forEach((c) => c());"], ["index", "cbs1[0]();"], ["const", "fn1();"], ["mapget", 'reg1.get("k")!();'], ["bus", "bus1.emit();"], ["promise", "await Promise.all(tasks1.map((t) => t()));"], ["param", "run1(hx1);"]]) {
      for (const host of SF_HOSTS) {
        const lines = run(`sf-${form}-${host}`);
        expect(lines, `${form}/${host}`).toContain(invocation);
        // the stored function is a function of the module, the invocation is behind the compute's await
        expect(lines.some((line) => /^(async )?function hx1\(\) \{$/.test(line)), `${form}/${host}`).toBe(true);
        expect(lines.indexOf(invocation), `${form}/${host}`).toBeGreaterThan(lines.indexOf("await Promise.resolve();"));
      }
    }
    // how each form stores it
    expect(run("sf-foreach-memo-async")).toContain("cbs1.push(hx1);");
    expect(run("sf-mapget-memo-async")).toContain('reg1.set("k", hx1);');
    expect(run("sf-bus-memo-async")).toContain("bus1.listeners.push(hx1);");
    expect(run("sf-const-memo-async")).toContain("const fn1 = cbs1[0];");
    expect(run("sf-param-memo-async")).toContain("function run1(f: () => unknown) { f(); };");
  });
  it("keep the twin that never invokes it: the names are used and nothing calls", () => {
    for (const form of SF_FORM_NAMES) for (const host of SF_HOSTS) {
      const live = run(`sf-${form}-${host}`);
      const never = run(`sf-${form}-${host}-never`);
      expect(never.some((line) => /^void [A-Za-z]\w*;$/.test(line)), `${form}/${host}`).toBe(true);
      // the invocation is what the twin leaves out
      const invoked = live.filter((line) => !never.includes(line) && !/^void /.test(line));
      expect(invoked.length, `${form}/${host}`).toBeGreaterThan(0);
      expect(invoked.some((line) => /\(\)|hx1\)/.test(line)), `${form}/${host}`).toBe(true);
      expect(never.filter((line) => !live.includes(line)), `${form}/${host}`).toEqual(never.filter((line) => /^void [A-Za-z]\w*;$/.test(line)));
    }
  });
  it("keep the twin that invokes it before the await, where the read is a dependency", () => {
    for (const form of SF_FORM_NAMES) for (const host of SF_HOSTS) {
      const before = run(`sf-${form}-${host}-before`);
      const live = run(`sf-${form}-${host}`);
      const awaitAt = before.indexOf("await Promise.resolve();");
      expect(awaitAt, `${form}/${host}`).toBeGreaterThanOrEqual(0);
      const invoked = live.filter((line) => !before.slice(0, awaitAt).includes(line) && !before.includes(line));
      expect(invoked, `${form}/${host}`).toEqual([]);
      // a statement of the invocation stands ahead of the await in the twin and behind it in the program
      const trigger = live.slice(live.indexOf("await Promise.resolve();") + 1).find((line) => /\(\)|hx1\)/.test(line));
      expect(before.indexOf(trigger), `${form}/${host}`).toBeGreaterThanOrEqual(0);
      expect(before.indexOf(trigger), `${form}/${host}`).toBeLessThan(awaitAt);
    }
  });
  it("every cell builds a neutral, valid program of one site", () => {
    for (const context of SF_CONTEXT_NAMES) {
      const { program, built } = textOf(context);
      expect(isValid(program), context).toBe(true);
      expect(built.neutral, context).toBe(true);
      expect(built.sites, context).toHaveLength(1);
    }
  });
});
