import { describe, expect, it } from "vitest";
import { buildCase } from "./emit.mjs";
import { generateProgram } from "./generate.mjs";
import { recallLegalShapes } from "./legal.mjs";
import { BASE_ONLY_CONTEXTS, SPEC } from "./misuse.mjs";
import { censusCells, recallShapes } from "./matrix.mjs";
import { isValid } from "./model.mjs";
import { EM_CODES, EM_CONTEXTS, EM_CONTEXT_NAMES, EM_HOSTS, EM_ROUTES, emParts, emShape } from "./em-shapes.mjs";
import { checkerProjectFiles } from "./checker.mjs";

// Batch 11 (the follow-up to the review of #220, and of #221): a function that returns JSX, held by a variable, handed to a wrapper or reached by `eval`, and called where no owner is current, as generator cells with their twins.

const builtOf = (code, context) => {
  const side = EM_CONTEXTS[context];
  const shape = (side === "misuse" ? recallShapes : recallLegalShapes)(code, context).find((one) => one.binding === "direct" && one.layer === "none");
  const program = generateProgram(7, { profile: "misuse", misusePlan: { code, context, variant: shape.variant, binding: "direct", layer: "none", legal: side === "legal" } });
  return buildCase(program, { id: `em-${context}` });
};

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
  it("are nine routes on three hosts, each with a twin that never runs the site, each name parsed back", () => {
    expect(EM_ROUTES).toEqual(["view", "gen", "self", "hoc", "eval", "ns", "nss", "nsa", "nso"]);
    expect(EM_HOSTS).toEqual(["handler", "timer", "apply"]);
    expect(EM_CONTEXT_NAMES).toHaveLength(54);
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
  it("print the hoc route: the arrow that returns JSX is the argument of a wrapper that keeps it, a tag renders the binding, the host calls the kept function (the twin only keeps the holder)", () => {
    for (const host of EM_HOSTS) {
      const lead = textOf("NO_OWNER_CLEANUP", `em-hoc-${host}`);
      expect(lead, host).toContain("let sv1: (() => unknown) | undefined;");
      expect(lead, host).toContain("const wr1 = (c: () => unknown) => { sv1 = c; return c as () => any; };");
      expect(lead, host).toContain("const Wr1 = wr1(() => {");
      expect(lead.join("\n"), host).toContain("return <p />;");
      expect(lead.join("\n"), host).toContain("<Wr1 />");
      expect(lead, host).toContain("sv1!();");
      const twin = textOf("NO_OWNER_CLEANUP", `em-hoc-${host}-never`);
      expect(twin, host).toContain("void sv1;");
      expect(twin, host).not.toContain("sv1!();");
    }
  });
  it("print the eval route: the component a tag renders, the host reaches it by name through direct eval (the twin evaluates 1)", () => {
    for (const host of EM_HOSTS) {
      const lead = textOf("NO_OWNER_CLEANUP", `em-eval-${host}`);
      expect(lead.join("\n"), host).toContain("function K1() {");
      expect(lead.join("\n"), host).toContain("return <p />;");
      expect(lead.join("\n"), host).toContain("<K1 />");
      expect(lead, host).toContain('eval("K1()");');
      const twin = textOf("NO_OWNER_CLEANUP", `em-eval-${host}-never`);
      expect(twin, host).toContain('eval("1");');
      expect(twin.join("\n"), host).not.toContain('eval("K1()")');
    }
  });
  it("print the ns route: an exported component a tag renders, called by the host through the module's own namespace by a key built at run time (the twin only keeps the namespace used)", () => {
    for (const host of EM_HOSTS) {
      const lead = textOf("NO_OWNER_CLEANUP", `em-ns-${host}`);
      expect(lead, host).toContain('import * as self from "./App";');
      expect(lead.join("\n"), host).toContain("export function K1() {");
      expect(lead.join("\n"), host).toContain("<K1 />");
      expect(lead, host).toContain('self[["K","1"].join("") as "K1"]();');
      const twin = textOf("NO_OWNER_CLEANUP", `em-ns-${host}-never`);
      expect(twin, host).toContain("void self;");
      expect(twin.join("\n"), host).not.toContain('self[["K"');
    }
  });
  it("print the nss and nsa routes: the component published by an export specifier (R1), a plain one and a renamed arrow, called through the module's namespace by a key built at run time", () => {
    for (const host of EM_HOSTS) {
      const lead = textOf("NO_OWNER_CLEANUP", `em-nss-${host}`);
      expect(lead, host).toContain("function K1() {");
      expect(lead, host).toContain("export { K1 };");
      expect(lead.join("\n"), host).not.toContain("export function K1");
      expect(lead, host).toContain('self[["K","1"].join("") as "K1"]();');
      const alias = textOf("NO_OWNER_CLEANUP", `em-nsa-${host}`);
      expect(alias, host).toContain("const K1 = () => {");
      expect(alias, host).toContain("export { K1 as Out1 };");
      expect(alias, host).toContain('self[["O","ut1"].join("") as "Out1"]();');
      expect(alias.join("\n"), host).toContain("<K1 />");
      for (const route of ["nss", "nsa"]) {
        const twin = textOf("NO_OWNER_CLEANUP", `em-${route}-${host}-never`);
        expect(twin, `${route} ${host}`).toContain("void self;");
        expect(twin.join("\n"), `${route} ${host}`).not.toContain("self[[");
      }
    }
  });
  it("print the nso route: the ns route in an OPEN program, which the oracle's programBoundary and the checker's manifest carry (R2)", () => {
    for (const host of EM_HOSTS) {
      const open = builtOf("NO_OWNER_CLEANUP", `em-nso-${host}`);
      expect(open.text, host).toContain("export function K1() {");
      expect(JSON.parse(open.files["oracle.json"]).programBoundary, host).toBe("open");
      expect(JSON.parse(open.files["oracle.run.json"]).programBoundary, host).toBe("open");
      expect(JSON.parse(checkerProjectFiles(open)["package.json"]), host).toEqual({ name: `em-em-nso-${host}`, main: "main.tsx" });
      const closed = builtOf("NO_OWNER_CLEANUP", `em-ns-${host}`);
      expect(JSON.parse(closed.files["oracle.json"]).programBoundary, host).toBeUndefined();
      expect(checkerProjectFiles(closed)["package.json"], host).toBeUndefined();
    }
  });
  it("are cells of SPEC on their side for the codes the host reports, one base shape, not the census's, out of api-fire's plan", () => {
    expect(emShape("em-view-timer", { binding: "alias", layer: "none" })).toBe(false);
    expect(emShape("pl-untrack-tracked", { binding: "alias", layer: "iife" })).toBe(true);
    expect(emShape("em-ns-timer", { binding: "direct", layer: "none", variant: "reaction-track-owned" })).toBe(false);
    expect(emShape("em-eval-timer", { binding: "direct", layer: "none", variant: "reaction-track-owned" })).toBe(true);
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
