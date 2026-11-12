import { describe, expect, it } from "vitest";
import { buildCase } from "./emit.mjs";
import { generateProgram } from "./generate.mjs";
import { recallLegalShapes } from "./legal.mjs";
import { BASE_ONLY_CONTEXTS, SPEC } from "./misuse.mjs";
import { censusCells, recallShapes } from "./matrix.mjs";
import { isValid } from "./model.mjs";
import { ER_CODES, ER_CONTEXTS, ER_CONTEXT_NAMES, ER_HOSTS, ER_ROUTES, erParts, erShape } from "./er-shapes.mjs";

// Batch 11, slice 6 (the review of #220, F1-F3): a function the host's callback reaches by a reference, as generator cells with their twins.

const textOf = (code, context) => {
  const side = ER_CONTEXTS[context];
  const shape = (side === "misuse" ? recallShapes : recallLegalShapes)(code, context).find((one) => one.binding === "direct" && one.layer === "none");
  const program = generateProgram(7, { profile: "misuse", misusePlan: { code, context, variant: shape.variant, binding: "direct", layer: "none", legal: side === "legal" } });
  const built = buildCase(program, { id: `er-${context}` });
  expect(isValid(program), context).toBe(true);
  expect(built.neutral, context).toBe(true);
  expect(built.sites, context).toHaveLength(1);
  return built.text.split("\n").map((line) => line.trim());
};

describe("the er contexts (a function reached by a reference)", () => {
  it("are six routes on an apply and the view route on the two leaf hosts, each with a twin that never runs the site, each name parsed back", () => {
    expect(ER_ROUTES).toEqual(["view", "array", "saved", "timer", "generator", "root"]);
    expect(ER_HOSTS).toEqual(["apply", "tracked", "settled"]);
    expect(ER_CONTEXT_NAMES).toHaveLength(2 * (6 + 2));
    for (const name of ER_CONTEXT_NAMES) {
      const parts = erParts(name);
      expect(parts, name).not.toBeNull();
      expect(ER_CONTEXTS[name]).toBe(parts.never ? "legal" : "misuse");
      expect(name).toBe(`er-${parts.route}-${parts.host}${parts.never ? "-never" : ""}`);
    }
    for (const other of ["er-view", "er-array-tracked", "er-root-settled", "la-let-later-tracked", "ph-ref-leaf"]) expect(erParts(other), other).toBeNull();
  });
  it("print the view route: a function that returns JSX, no tag renders it, held by a variable the host calls (or only keeps)", () => {
    const lead = textOf("PRIMITIVE_IN_EFFECT_CALLBACK", "er-view-apply");
    const at = lead.findIndex((line) => line.startsWith("function h1()"));
    expect(at).toBeGreaterThan(-1);
    expect(lead.slice(at, at + 4).join(" ")).toContain("return <span />;");
    expect(lead).toContain("const pk1: (() => unknown) | undefined = h1;");
    expect(lead).toContain("pk1!();");
    const twin = textOf("PRIMITIVE_IN_EFFECT_CALLBACK", "er-view-apply-never");
    expect(twin).toContain("void pk1;");
    expect(twin).not.toContain("pk1!();");
    expect(textOf("CLEANUP_IN_FORBIDDEN_SCOPE", "er-view-tracked")).toContain("pk1!();");
    expect(textOf("PRIMITIVE_IN_FORBIDDEN_SCOPE", "er-view-settled")).toContain("pk1!();");
  });
  it("print the array, saved and timer routes: the registration, the second holder, the host's call", () => {
    const array = textOf("PRIMITIVE_IN_EFFECT_CALLBACK", "er-array-apply");
    expect(array).toContain("const all1 = [h1];");
    expect(array).toContain("all1[0]!();");
    expect(array.join("\n")).toContain("onClick={h1}");
    const saved = textOf("PRIMITIVE_IN_EFFECT_CALLBACK", "er-saved-apply");
    expect(saved).toContain("pk1 = h1;");
    expect(saved).toContain("pk1!();");
    expect(saved.join("\n")).toContain("onClick={h1}");
    const timer = textOf("PRIMITIVE_IN_EFFECT_CALLBACK", "er-timer-apply");
    expect(timer).toContain("setTimeout(h1, 1000);");
    expect(timer).toContain("pk1!();");
    for (const route of ["array", "saved", "timer"]) {
      const twin = textOf("PRIMITIVE_IN_EFFECT_CALLBACK", `er-${route}-apply-never`).join("\n");
      expect(twin, route).not.toContain("onClick");
      expect(twin, route).not.toContain("setTimeout");
      expect(twin, route).not.toMatch(/!\(\);/);
    }
  });
  it("print the generator route: a generator literal untrack hands back, stepped by the apply (the twin only keeps the iterator)", () => {
    const lead = textOf("PRIMITIVE_IN_EFFECT_CALLBACK", "er-generator-apply");
    expect(lead).toContain("const it1 = untrack(function* () {");
    expect(lead).toContain("yield 1;");
    expect(lead).toContain("it1.next();");
    const twin = textOf("PRIMITIVE_IN_EFFECT_CALLBACK", "er-generator-apply-never");
    expect(twin).toContain("void it1;");
    expect(twin).not.toContain("it1.next();");
  });
  it("print the root route: a generator callback of createRoot in a memo, stepped by the apply (the twin only keeps the memo)", () => {
    const lead = textOf("PRIMITIVE_IN_EFFECT_CALLBACK", "er-root-apply");
    expect(lead).toContain("const m1 = createMemo(() => createRoot(function* () {");
    expect(lead).toContain("yield 1;");
    expect(lead).toContain("createEffect(() => m1(), (it) => { it.next(); });");
    const twin = textOf("PRIMITIVE_IN_EFFECT_CALLBACK", "er-root-apply-never");
    expect(twin).toContain("createEffect(() => m1(), () => { void m1; });");
    expect(twin.join("\n")).not.toContain("it.next()");
  });
  it("are cells of SPEC on their side for the codes their host reports, one base shape, not the census's, out of api-fire's plan", () => {
    expect(erShape("er-view-apply", { binding: "alias", layer: "none" })).toBe(false);
    expect(erShape("pl-untrack-tracked", { binding: "alias", layer: "iife" })).toBe(true);
    expect(Object.keys(ER_CODES)).toEqual(["PRIMITIVE_IN_EFFECT_CALLBACK", "PRIMITIVE_IN_FORBIDDEN_SCOPE", "CLEANUP_IN_FORBIDDEN_SCOPE"]);
    for (const [code, { hosts, routes }] of Object.entries(ER_CODES)) {
      for (const context of ER_CONTEXT_NAMES.filter((name) => hosts.includes(erParts(name).host) && routes.includes(erParts(name).route))) {
        const side = ER_CONTEXTS[context] === "misuse" ? SPEC[code].recallContexts : SPEC[code].recallLegal;
        expect(side, `${code} ${context}`).toContain(context);
        const shapes = (ER_CONTEXTS[context] === "misuse" ? recallShapes : recallLegalShapes)(code, context);
        expect(shapes.length, `${code} ${context}`).toBeGreaterThan(0);
        for (const shape of shapes) expect({ binding: shape.binding, layer: shape.layer ?? "none" }, `${code} ${context}`).toEqual({ binding: "direct", layer: "none" });
      }
    }
    for (const context of ER_CONTEXT_NAMES) expect(BASE_ONLY_CONTEXTS()).toContain(context);
    expect(censusCells().filter(({ context }) => ER_CONTEXT_NAMES.includes(context))).toEqual([]);
  });
});
