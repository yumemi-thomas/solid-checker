import { describe, expect, it } from "vitest";
import { buildCase } from "./emit.mjs";
import { generateProgram } from "./generate.mjs";
import { recallLegalShapes } from "./legal.mjs";
import { BASE_ONLY_CONTEXTS, SPEC } from "./misuse.mjs";
import { censusCells, recallShapes } from "./matrix.mjs";
import { isValid } from "./model.mjs";
import { FC_CODES, FC_CONTEXTS, FC_CONTEXT_NAMES, FC_FORMS, FC_HOSTS, fcParts, fcShape } from "./fc-shapes.mjs";

// Batch 11 (the review of #213): a flush in a named function expression a host enters again by its name, and in the parameter default of a function the host calls, as generator cells with their twins.

const textOf = (code, context) => {
  const side = FC_CONTEXTS[context];
  const shape = (side === "misuse" ? recallShapes : recallLegalShapes)(code, context).find((one) => one.binding === "direct" && one.layer === "none");
  const program = generateProgram(7, { profile: "misuse", misusePlan: { code, context, variant: shape.variant, binding: "direct", layer: "none", legal: side === "legal" } });
  const built = buildCase(program, { id: `fc-${context}` });
  expect(isValid(program), context).toBe(true);
  expect(built.neutral, context).toBe(true);
  expect(built.sites, context).toHaveLength(1);
  return built.text.split("\n").map((line) => line.trim());
};

describe("the fc contexts (a flush in a function the host enters again, or calls for its default)", () => {
  it("are three forms on three hosts, each with a twin that never runs the site, each name parsed back", () => {
    expect(FC_FORMS).toEqual(["named", "pdefault", "pdefault-await"]);
    expect(FC_HOSTS).toEqual(["effect-apply", "action", "tracked"]);
    expect(FC_CONTEXT_NAMES).toHaveLength(18);
    for (const name of FC_CONTEXT_NAMES) {
      const parts = fcParts(name);
      expect(parts, name).not.toBeNull();
      expect(FC_CONTEXTS[name]).toBe(parts.never ? "legal" : "misuse");
      expect(name).toBe(`fc-${parts.form}-${parts.host}${parts.never ? "-never" : ""}`);
    }
    for (const other of ["fc-named", "fc-named-body", "la-let-later-tracked", "nm-async-tracked", "pl-untrack-tracked"]) expect(fcParts(other), other).toBeNull();
  });
  it("print the named form: the site first, then the host calls the name from its own stack, or runs nothing in the twin", () => {
    const lead = textOf("FLUSH_IN_ACTION", "fc-named-action");
    expect(lead[lead.findIndex((line) => line.startsWith("setTimeout(function tick"))]).toMatch(/^setTimeout\(function tick\d+\(again\d+\?: boolean\) \{$/);
    expect(lead.indexOf("flush();")).toBeLessThan(lead.findIndex((line) => /^tick\d+\(true\);$/.test(line)));
    expect(lead).toContain("void fr1();");
    const twin = textOf("FLUSH_IN_ACTION", "fc-named-action-never");
    expect(twin.some((line) => /^tick\d+\(true\);$/.test(line))).toBe(false);
    expect(twin).toContain("flush();");
    expect(textOf("FLUSH_IN_EFFECT_CALLBACK", "fc-named-effect-apply").some((line) => line.startsWith("createEffect("))).toBe(true);
    expect(textOf("FLUSH_IN_TRACKED_QUEUE_CALLBACK", "fc-named-tracked").some((line) => line.startsWith("createTrackedEffect("))).toBe(true);
  });
  it("print the default forms: the site is the default of a function a timer's callback (or the code after an await) assigns, and the host calls it", () => {
    const lead = textOf("FLUSH_IN_EFFECT_CALLBACK", "fc-pdefault-effect-apply");
    expect(lead).toContain("late1 = (x = flush()) => x;");
    expect(lead).toContain("late1();");
    const twin = textOf("FLUSH_IN_EFFECT_CALLBACK", "fc-pdefault-effect-apply-never");
    expect(twin).toContain("late1 = (x = flush()) => x;");
    expect(twin).not.toContain("late1();");
    expect(twin).toContain("void late1;");
    const awaited = textOf("FLUSH_IN_TRACKED_QUEUE_CALLBACK", "fc-pdefault-await-tracked");
    expect(awaited.indexOf("await Promise.resolve();")).toBeLessThan(awaited.indexOf("late1 = (x = flush()) => x;"));
    expect(textOf("FLUSH_IN_ACTION", "fc-pdefault-action")).toContain("void fr1();");
  });
  it("are cells of SPEC on their side for the codes their host reports, one base shape, not the census's, out of api-fire's plan", () => {
    expect(fcShape("fc-named-action", { binding: "alias", layer: "none" })).toBe(false);
    expect(fcShape("pl-untrack-tracked", { binding: "alias", layer: "iife" })).toBe(true);
    for (const [code, hosts] of Object.entries(FC_CODES)) {
      for (const context of FC_CONTEXT_NAMES.filter((name) => hosts.includes(fcParts(name).host))) {
        const side = FC_CONTEXTS[context] === "misuse" ? SPEC[code].recallContexts : SPEC[code].recallLegal;
        expect(side, `${code} ${context}`).toContain(context);
        const shapes = (FC_CONTEXTS[context] === "misuse" ? recallShapes : recallLegalShapes)(code, context);
        expect(shapes.length, `${code} ${context}`).toBeGreaterThan(0);
        for (const shape of shapes) expect({ binding: shape.binding, layer: shape.layer ?? "none" }, `${code} ${context}`).toEqual({ binding: "direct", layer: "none" });
      }
    }
    for (const context of FC_CONTEXT_NAMES) expect(BASE_ONLY_CONTEXTS()).toContain(context);
    expect(Object.keys(FC_CODES)).toEqual(["FLUSH_IN_EFFECT_CALLBACK", "FLUSH_IN_ACTION", "FLUSH_IN_TRACKED_QUEUE_CALLBACK"]);
    expect(censusCells().filter(({ context }) => FC_CONTEXT_NAMES.includes(context))).toEqual([]);
  });
});
