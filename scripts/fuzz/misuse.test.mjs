import { describe, expect, it } from "vitest";
import { codePolicy, codeRules, policyOf } from "./codes.mjs";
import { generateProgram } from "./generate.mjs";
import { stripInstrumentation } from "./hits.mjs";
import { BINDINGS, LAYERS, MISUSE_CODES, SPEC, TEMPLATE_CODES } from "./misuse.mjs";
import { validate } from "./model.mjs";
import { printProgram } from "./print.mjs";

const plan = (code, extra = {}) => ({ code, ...extra });
const cellList = () => TEMPLATE_CODES.flatMap((code) => SPEC[code].contexts.map((context) => ({ code, context })));

describe("the misuse spec", () => {
  it("covers every claimed code of the policy table once, and the unclaimed defects that have a template", () => {
    const claimed = Object.entries(codePolicy).filter(([, policy]) => policy.disposition === "defect-claimed").map(([code]) => code);
    expect(MISUSE_CODES.filter((code) => policyOf(code).disposition === "defect-claimed").sort()).toEqual([...claimed].sort());
    expect(MISUSE_CODES.filter((code) => policyOf(code).disposition === "defect-unclaimed").sort()).toEqual(["ASYNC_STORE_SETTER", "PRIMITIVE_IN_EFFECT_CALLBACK"]);
    expect(Object.keys(codeRules).filter((code) => policyOf(code).disposition === "defect-claimed" && !SPEC[code])).toEqual([]);
  });
  it("gives every code with no template the reason the typings reject it", () => {
    const typedAway = MISUSE_CODES.filter((code) => SPEC[code].typedAway);
    expect(typedAway).toEqual(["MISSING_EFFECT_FN", "ASYNC_STORE_SETTER"]);
    for (const code of typedAway) { expect(SPEC[code].why).toMatch(/typings reject/); expect(SPEC[code].probe).toContain("export function"); }
  });
  it("lists the contexts, bindings and a call for every template", () => {
    for (const code of TEMPLATE_CODES) {
      expect(SPEC[code].contexts.length, code).toBeGreaterThan(0);
      expect(SPEC[code].bindings.every((binding) => BINDINGS.includes(binding)), code).toBe(true);
    }
  });
});

describe("the misuse generator", () => {
  it("builds valid, neutral programs with exactly one marked site, for every cell, binding and layer", () => {
    for (const { code, context } of cellList()) {
      for (const binding of SPEC[code].bindings) {
        for (const layer of LAYERS) {
          const program = generateProgram(7, { profile: "misuse", misusePlan: plan(code, { context, binding, layer }) });
          expect(validate(program), `${code}/${context}/${binding}/${layer}`).toEqual([]);
          const printed = printProgram(program);
          expect(printed.sites, `${code}/${context}/${binding}/${layer}`).toHaveLength(1);
          expect(printed.sites[0].code).toBe(code);
          expect(stripInstrumentation(printed.runText), `${code}/${context}/${binding}/${layer}`).toBe(printed.text);
        }
      }
    }
  });
  it("is a pure function of the seed and the plan", () => {
    const a = generateProgram(3, { profile: "misuse", misusePlan: plan("FLUSH_IN_ACTION") });
    expect(generateProgram(3, { profile: "misuse", misusePlan: plan("FLUSH_IN_ACTION") })).toEqual(a);
  });
  it("refuses a typed-away code and a context the code does not have", () => {
    expect(() => generateProgram(1, { profile: "misuse", misusePlan: plan("MISSING_EFFECT_FN") })).toThrow(/no misuse template/);
    expect(() => generateProgram(1, { profile: "misuse", misusePlan: plan("FLUSH_IN_ACTION", { context: "memo" }) })).toThrow(/no memo context/);
  });
  it("records a wrapper's inner call line next to the site", () => {
    const program = generateProgram(2, { profile: "misuse", misusePlan: plan("NO_OWNER_CLEANUP", { context: "handler", binding: "wrapper", layer: "none" }) });
    const [site] = printProgram(program).sites;
    expect(site.aux).toHaveLength(1);
    expect(site.aux[0]).not.toBe(site.line);
  });
  it("draws each binding form into the printed text", () => {
    const text = (binding) => printProgram(generateProgram(2, { profile: "misuse", misusePlan: plan("NO_OWNER_CLEANUP", { context: "timer", binding, layer: "none" }) })).text;
    expect(text("direct")).toContain("onCleanup(() => {})");
    expect(text("namespace")).toContain("S.onCleanup(");
    expect(text("alias")).toContain("onCleanupAlias(");
    expect(text("destructured")).toContain("const { onCleanup: onCleanupD } = S;");
    expect(text("wrapper")).toContain("wrapUse()");
  });
});
