import { describe, expect, it } from "vitest";
import { buildCase } from "./emit.mjs";
import { generateProgram } from "./generate.mjs";
import { recallLegalShapes } from "./legal.mjs";
import { BASE_ONLY_CONTEXTS, SPEC } from "./misuse.mjs";
import { censusCells, recallShapes } from "./matrix.mjs";
import { isValid } from "./model.mjs";
import { FR_CODES, FR_CONTEXTS, FR_CONTEXT_NAMES, FR_FORMS, FR_HOSTS, frParts, frPlacement, frShape } from "./fr-shapes.mjs";
import { plans } from "./api-fire-plan.mjs";

// ADR 0990: a primitive (`flush`, `resolve`, `until`) handed BY REFERENCE, as generator cells with the twin whose function nothing calls.

const shapesOf = (code, context) => (FR_CONTEXTS[context] === "misuse" ? recallShapes : recallLegalShapes)(code, context);
const textOf = (code, context) => {
  const side = FR_CONTEXTS[context];
  const shape = shapesOf(code, context).find((one) => one.binding === "direct" && one.layer === "none");
  const program = generateProgram(7, { profile: "misuse", misusePlan: { code, context, variant: shape.variant, binding: "direct", layer: "none", legal: side === "legal" } });
  const built = buildCase(program, { id: `fr-${context}` });
  expect(isValid(program), `${code}/${context}`).toBe(true);
  expect(built.neutral, `${code}/${context}`).toBe(true);
  expect(built.sites, `${code}/${context}`).toHaveLength(1);
  return built.text.split("\n").map((line) => line.trim());
};

describe("the fr contexts (a primitive handed by reference)", () => {
  it("are four forms of flush on three hosts and three of resolve and until on the memo (one set of names for both codes), each with its twin, each name parsed back", () => {
    expect(FR_FORMS).toEqual(["untrack", "alias", "element", "param"]);
    expect(FR_HOSTS).toEqual(["action", "tracked", "effect-apply", "memo"]);
    expect(FR_CONTEXT_NAMES).toHaveLength(3 * 4 * 2 + 3 * 2);
    for (const name of FR_CONTEXT_NAMES) {
      const parts = frParts(name);
      expect(parts, name).not.toBeNull();
      expect(FR_CONTEXTS[name]).toBe(parts.never ? "legal" : "misuse");
      expect(name).toBe(`fr-${parts.form}-${parts.host}${parts.never ? "-never" : ""}`);
    }
    for (const other of ["fr-untrack-body", "fr-flush-action", "fr-untrack-action-never-never", "pl-untrack-action", "nm-async-tracked"]) expect(frParts(other), other).toBeNull();
  });
  it("take `untrack(flush)` for flush only: `resolve` and `until` need an argument, so `untrack(resolve)` is a type error", () => {
    for (const code of ["RESOLVE_IN_REACTIVE_SCOPE", "UNTIL_IN_REACTIVE_SCOPE"]) expect(FR_CODES[code].forms).not.toContain("untrack");
    for (const code of ["FLUSH_IN_ACTION", "FLUSH_IN_TRACKED_QUEUE_CALLBACK", "FLUSH_IN_EFFECT_CALLBACK"]) expect(FR_CODES[code].forms).toEqual(FR_FORMS);
  });
  it("print each form's site in the host, one marked site, the primitive imported once", () => {
    expect(textOf("FLUSH_IN_ACTION", "fr-untrack-action")).toContain("untrack(flush);");
    const alias = textOf("FLUSH_IN_ACTION", "fr-alias-action");
    expect(alias).toContain("const fa1 = flush;");
    expect(alias).toContain("fa1();");
    expect(alias.indexOf("const fa1 = flush;")).toBeLessThan(alias.indexOf("fa1();"));
    expect(textOf("FLUSH_IN_TRACKED_QUEUE_CALLBACK", "fr-element-tracked")).toContain("[flush][0]();");
    const param = textOf("FLUSH_IN_EFFECT_CALLBACK", "fr-param-effect-apply");
    expect(param).toContain("frCall(flush);");
    expect(param.some((line) => line.startsWith("function frCall(f: () => unknown)"))).toBe(true);
    expect(textOf("RESOLVE_IN_REACTIVE_SCOPE", "fr-element-memo")).toContain("[resolve][0](() => 1);");
    expect(textOf("UNTIL_IN_REACTIVE_SCOPE", "fr-alias-memo")).toContain("const fa1 = until;");
    expect(textOf("UNTIL_IN_REACTIVE_SCOPE", "fr-param-memo")).toContain("frCall(until);");
    for (const context of FR_CONTEXT_NAMES.filter((name) => !name.endsWith("-never"))) {
      const code = Object.keys(FR_CODES).find((one) => frPlacement()[one].misuse.includes(context));
      const text = textOf(code, context);
      expect(text.filter((line) => /^import .*solid-js/.test(line)), `${code}/${context}`).toHaveLength(1);
    }
  });
  it("print the twin's text in a function the host only names", () => {
    const never = textOf("FLUSH_IN_ACTION", "fr-untrack-action-never");
    expect(never).toContain("const fz1 = () => {");
    expect(never).toContain("untrack(flush);");
    expect(never).toContain("void fz1;");
    expect(never).not.toContain("fz1();");
    const alias = textOf("RESOLVE_IN_REACTIVE_SCOPE", "fr-alias-memo-never");
    expect(alias.indexOf("const fz1 = () => {")).toBeLessThan(alias.indexOf("const fa1 = resolve;"));
    expect(alias).toContain("void fz1;");
  });
  it("are cells of SPEC on their side, one base shape, not the census's, out of api-fire's plan", () => {
    expect(frShape("fr-alias-action", { binding: "alias", layer: "none" })).toBe(false);
    expect(frShape("fr-alias-action", { binding: "direct", layer: "none" })).toBe(true);
    expect(frShape("pl-untrack-action", { binding: "alias", layer: "iife" })).toBe(true);
    for (const [code, { misuse, legal }] of Object.entries(frPlacement())) {
      for (const context of misuse) { expect(SPEC[code].recallContexts, `${code}/${context}`).toContain(context); expect(recallShapes(code, context), `${code}/${context}`).toHaveLength(1); expect(BASE_ONLY_CONTEXTS()).toContain(context); }
      for (const context of legal) { expect(SPEC[code].recallLegal, `${code}/${context}`).toContain(context); expect(recallLegalShapes(code, context), `${code}/${context}`).toHaveLength(1); }
    }
    expect(Object.keys(frPlacement()).sort()).toEqual(Object.keys(FR_CODES).sort());
    expect(censusCells().filter(({ context }) => FR_CONTEXT_NAMES.includes(context))).toEqual([]);
    expect(plans().filter((plan) => FR_CONTEXT_NAMES.includes(plan.context))).toEqual([]);
  });
});
