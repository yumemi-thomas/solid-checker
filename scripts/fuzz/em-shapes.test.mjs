import { describe, expect, it } from "vitest";
import { buildCase } from "./emit.mjs";
import { generateProgram } from "./generate.mjs";
import { recallLegalShapes } from "./legal.mjs";
import { BASE_ONLY_CONTEXTS, SPEC } from "./misuse.mjs";
import { censusCells, recallShapes } from "./matrix.mjs";
import { isValid } from "./model.mjs";
import { EM_CODES, EM_CONTEXTS, EM_CONTEXT_NAMES, EM_HOSTS, EM_ROUTES, emParts, emShape } from "./em-shapes.mjs";

// Batch 11 (the follow-up to the review of #220): a function that returns JSX, held by a variable and called where no owner is current, as generator cells with their twins.

const textOf = (code, context) => {
  const side = EM_CONTEXTS[context];
  const shape = (side === "misuse" ? recallShapes : recallLegalShapes)(code, context).find((one) => one.binding === "direct" && one.layer === "none");
  const program = generateProgram(7, { profile: "misuse", misusePlan: { code, context, variant: shape.variant, binding: "direct", layer: "none", legal: side === "legal" } });
  const built = buildCase(program, { id: `em-${context}` });
  expect(isValid(program), context).toBe(true);
  expect(built.neutral, context).toBe(true);
  expect(built.sites, context).toHaveLength(1);
  return built.text.split("\n").map((line) => line.trim());
};

describe("the em contexts (a JSX-returning function a variable holds, called where no owner is current)", () => {
  it("are three routes on three hosts, each with a twin that never runs the site, each name parsed back", () => {
    expect(EM_ROUTES).toEqual(["view", "gen", "self"]);
    expect(EM_HOSTS).toEqual(["handler", "timer", "apply"]);
    expect(EM_CONTEXT_NAMES).toHaveLength(18);
    for (const name of EM_CONTEXT_NAMES) {
      const parts = emParts(name);
      expect(parts, name).not.toBeNull();
      expect(EM_CONTEXTS[name]).toBe(parts.never ? "legal" : "misuse");
      expect(name).toBe(`em-${parts.route}-${parts.host}${parts.never ? "-never" : ""}`);
    }
    for (const other of ["em-view", "em-array-timer", "er-view-apply", "la-let-later-tracked"]) expect(emParts(other), other).toBeNull();
  });
  it("print the function that returns JSX, the holder, and the host's call (or only the holder's name in the twin)", () => {
    for (const [host, wrap] of [["handler", "onClick"], ["timer", "setTimeout"], ["apply", "createEffect"]]) {
      const lead = textOf("NO_OWNER_CLEANUP", `em-view-${host}`);
      const at = lead.findIndex((line) => line.startsWith("function h1()"));
      expect(at, host).toBeGreaterThan(-1);
      expect(lead.slice(at, at + 4).join(" "), host).toContain("return <span />;");
      expect(lead, host).toContain("const pk1: (() => unknown) | undefined = h1;");
      expect(lead, host).toContain("pk1!();");
      expect(lead.join("\n"), host).toContain(wrap);
      const twin = textOf("NO_OWNER_CLEANUP", `em-view-${host}-never`);
      expect(twin, host).toContain("void pk1;");
      expect(twin, host).not.toContain("pk1!();");
    }
    expect(textOf("NO_OWNER_EFFECT", "em-view-timer")).toContain("pk1!();");
    expect(textOf("NO_OWNER_CONTEXT", "em-view-apply")).toContain("pk1!();");
  });
  it("print the gen route: a helper beside a generator, called from its body, the iterator made in the body and stepped by the host (the twin only keeps the iterator)", () => {
    for (const host of EM_HOSTS) {
      const lead = textOf("NO_OWNER_CLEANUP", `em-gen-${host}`);
      expect(lead.join("\n"), host).toContain("function hh1() {");
      expect(lead, host).toContain("function* g1() { hh1(); yield 1; };");
      expect(lead, host).toContain("const it1 = g1();");
      expect(lead.join("\n"), host).toContain("for (const _ of it1) { void _; }");
      const twin = textOf("NO_OWNER_CLEANUP", `em-gen-${host}-never`);
      expect(twin, host).toContain("void it1;");
      expect(twin.join("\n"), host).not.toContain("for (const _ of");
    }
  });
  it("print the self route: a root's callback that is a named function expression saving itself, called by the host (the twin only keeps the holder)", () => {
    for (const host of EM_HOSTS) {
      const lead = textOf("NO_OWNER_CLEANUP", `em-self-${host}`);
      expect(lead, host).toContain("let sv1: (() => void) | undefined;");
      expect(lead.join("\n"), host).toContain("createRoot(function f1() {");
      expect(lead, host).toContain("if (!sv1) { sv1 = f1; return; };");
      expect(lead, host).toContain("sv1!();");
      const twin = textOf("NO_OWNER_CLEANUP", `em-self-${host}-never`);
      expect(twin, host).toContain("void sv1;");
      expect(twin, host).not.toContain("sv1!();");
    }
  });
  it("are cells of SPEC on their side for the codes the host reports, one base shape, not the census's, out of api-fire's plan", () => {
    expect(emShape("em-view-timer", { binding: "alias", layer: "none" })).toBe(false);
    expect(emShape("pl-untrack-tracked", { binding: "alias", layer: "iife" })).toBe(true);
    expect(Object.keys(EM_CODES)).toEqual(["NO_OWNER_CLEANUP", "NO_OWNER_EFFECT", "NO_OWNER_CONTEXT"]);
    for (const [code, hosts] of Object.entries(EM_CODES)) {
      for (const context of EM_CONTEXT_NAMES.filter((name) => hosts.includes(emParts(name).host))) {
        const side = EM_CONTEXTS[context] === "misuse" ? SPEC[code].recallContexts : SPEC[code].recallLegal;
        expect(side, `${code} ${context}`).toContain(context);
        const shapes = (EM_CONTEXTS[context] === "misuse" ? recallShapes : recallLegalShapes)(code, context);
        expect(shapes.length, `${code} ${context}`).toBeGreaterThan(0);
        for (const shape of shapes) expect({ binding: shape.binding, layer: shape.layer ?? "none" }, `${code} ${context}`).toEqual({ binding: "direct", layer: "none" });
      }
    }
    for (const context of EM_CONTEXT_NAMES) expect(BASE_ONLY_CONTEXTS()).toContain(context);
    expect(censusCells().filter(({ context }) => EM_CONTEXT_NAMES.includes(context))).toEqual([]);
  });
});
