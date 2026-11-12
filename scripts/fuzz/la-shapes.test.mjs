import { describe, expect, it } from "vitest";
import { buildCase } from "./emit.mjs";
import { generateProgram } from "./generate.mjs";
import { recallLegalShapes } from "./legal.mjs";
import { BASE_ONLY_CONTEXTS, SPEC } from "./misuse.mjs";
import { censusCells, recallShapes } from "./matrix.mjs";
import { isValid } from "./model.mjs";
import { LA_CODES, LA_CONTEXTS, LA_CONTEXT_NAMES, LA_FORMS, LA_HOSTS, laParts, laShape } from "./la-shapes.mjs";

// Batch 11: a function ASSIGNED to a variable, a member or an element after its declaration and called through the name, as generator cells with their twins.

const textOf = (code, context) => {
  const side = LA_CONTEXTS[context];
  const shape = (side === "misuse" ? recallShapes : recallLegalShapes)(code, context).find((one) => one.binding === "direct" && one.layer === "none");
  const program = generateProgram(7, { profile: "misuse", misusePlan: { code, context, variant: shape.variant, binding: "direct", layer: "none", legal: side === "legal" } });
  const built = buildCase(program, { id: `la-${context}` });
  expect(isValid(program), context).toBe(true);
  expect(built.neutral, context).toBe(true);
  expect(built.sites, context).toHaveLength(1);
  return built.text.split("\n").map((line) => line.trim());
};

describe("the la contexts (a function assigned later and called through its name)", () => {
  it("are six forms on four hosts, each with a twin that never runs the site, each name parsed back", () => {
    expect(LA_FORMS).toEqual(["let-later", "let-noinit", "let-other", "let-early", "member-later", "elem-later"]);
    expect(LA_HOSTS).toEqual(["effect-apply", "action", "tracked", "settled"]);
    expect(LA_CONTEXT_NAMES).toHaveLength(48);
    for (const name of LA_CONTEXT_NAMES) {
      const parts = laParts(name);
      expect(parts, name).not.toBeNull();
      expect(LA_CONTEXTS[name]).toBe(parts.never ? "legal" : "misuse");
      expect(name).toBe(`la-${parts.form}-${parts.host}${parts.never ? "-never" : ""}`);
    }
    for (const other of ["la-let-later", "la-let-later-body", "pl-untrack-tracked", "nm-async-tracked", "rs-prop-memo-async"]) expect(laParts(other), other).toBeNull();
  });
  it("print the lead and its twins: the function assigned inside the host's callback, then called, or never called", () => {
    const lead = textOf("FLUSH_IN_EFFECT_CALLBACK", "la-let-later-effect-apply");
    const at = lead.indexOf("let h1 = () => {};");
    expect(at).toBeGreaterThan(-1);
    expect(lead[at + 1]).toBe("h1 = () => {");
    expect(lead).toContain("flush();");
    expect(lead[lead.indexOf("h1();")]).toBe("h1();");
    expect(lead).not.toContain("void h1;");
    const idle = textOf("FLUSH_IN_EFFECT_CALLBACK", "la-let-later-effect-apply-never");
    expect(idle).toContain("void h1;");
    expect(idle).not.toContain("h1();");
    expect(textOf("FLUSH_IN_ACTION", "la-let-noinit-action")).toContain("let h1: () => void;");
    const other = textOf("FLUSH_IN_EFFECT_CALLBACK", "la-let-other-effect-apply");
    expect(other).toContain("const s1 = () => {");
    expect(other.indexOf("s1();")).toBeLessThan(other.indexOf("h1();"));
    const early = textOf("FLUSH_IN_EFFECT_CALLBACK", "la-let-early-effect-apply");
    expect(early.indexOf("h1();")).toBeLessThan(early.indexOf("h1 = () => {};"));
    const earlyTwin = textOf("FLUSH_IN_EFFECT_CALLBACK", "la-let-early-effect-apply-never");
    expect(earlyTwin.indexOf("h1 = () => {};")).toBeLessThan(earlyTwin.indexOf("h1();"));
    expect(textOf("FLUSH_IN_TRACKED_QUEUE_CALLBACK", "la-member-later-tracked")).toContain("o1.f = () => {");
    expect(textOf("FLUSH_IN_TRACKED_QUEUE_CALLBACK", "la-elem-later-settled")).toContain("a1[0] = () => {");
  });
  it("are cells of SPEC on their side for the codes their host reports, one base shape, not the census's, out of api-fire's plan", () => {
    expect(laShape("la-let-later-effect-apply", { binding: "alias", layer: "none" })).toBe(false);
    expect(laShape("pl-untrack-tracked", { binding: "alias", layer: "iife" })).toBe(true);
    for (const [code, hosts] of Object.entries(LA_CODES)) {
      for (const context of LA_CONTEXT_NAMES.filter((name) => hosts.includes(laParts(name).host))) {
        const side = LA_CONTEXTS[context] === "misuse" ? SPEC[code].recallContexts : SPEC[code].recallLegal;
        expect(side, `${code} ${context}`).toContain(context);
        // one base shape per variant of the code's own site (a code with three variants has three programs, all the direct binding bare)
        const shapes = (LA_CONTEXTS[context] === "misuse" ? recallShapes : recallLegalShapes)(code, context);
        expect(shapes.length, `${code} ${context}`).toBeGreaterThan(0);
        for (const shape of shapes) expect({ binding: shape.binding, layer: shape.layer ?? "none" }, `${code} ${context}`).toEqual({ binding: "direct", layer: "none" });
      }
    }
    for (const context of LA_CONTEXT_NAMES) expect(BASE_ONLY_CONTEXTS()).toContain(context);
    expect(Object.keys(LA_CODES)).toEqual(["FLUSH_IN_EFFECT_CALLBACK", "PRIMITIVE_IN_EFFECT_CALLBACK", "FLUSH_IN_ACTION", "FLUSH_IN_TRACKED_QUEUE_CALLBACK", "CLEANUP_IN_FORBIDDEN_SCOPE", "PRIMITIVE_IN_FORBIDDEN_SCOPE"]);
    expect(censusCells().filter(({ context }) => LA_CONTEXT_NAMES.includes(context))).toEqual([]);
  });
});
