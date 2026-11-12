import { describe, expect, it } from "vitest";
import { buildCase } from "./emit.mjs";
import { generateProgram } from "./generate.mjs";
import { recallLegalShapes } from "./legal.mjs";
import { BASE_ONLY_CONTEXTS, SPEC } from "./misuse.mjs";
import { censusCells, recallShapes } from "./matrix.mjs";
import { isValid } from "./model.mjs";
import { NM_CODES, NM_CONTEXTS, NM_CONTEXT_NAMES, NM_FORMS, NM_HOSTS, nmParts, nmShape } from "./nm-shapes.mjs";

// Slice 5's lead: a NAMED callback of `isPending` (a slot that takes what its callback throws), as generator cells with their twins.

const CODE = "FLUSH_IN_TRACKED_QUEUE_CALLBACK";
const textOf = (context) => {
  const side = NM_CONTEXTS[context];
  const shape = (side === "misuse" ? recallShapes : recallLegalShapes)(CODE, context).find((one) => one.binding === "direct" && one.layer === "none");
  const program = generateProgram(7, { profile: "misuse", misusePlan: { code: CODE, context, variant: shape.variant, binding: "direct", layer: "none", legal: side === "legal" } });
  const built = buildCase(program, { id: `nm-${context}` });
  expect(isValid(program), context).toBe(true);
  expect(built.neutral, context).toBe(true);
  expect(built.sites, context).toHaveLength(1);
  return built.text.split("\n").map((line) => line.trim());
};

describe("the nm contexts (a named callback of isPending)", () => {
  it("are six forms on the two leaf hosts, the async forms misuse and the others legal, each name parsed back", () => {
    expect(NM_FORMS).toEqual(["async", "wrap-async", "async-after", "sync", "wrap-sync", "gen"]);
    expect(NM_HOSTS).toEqual(["tracked", "settled"]);
    expect(NM_CONTEXT_NAMES).toHaveLength(12);
    for (const name of NM_CONTEXT_NAMES) {
      const parts = nmParts(name);
      expect(parts, name).not.toBeNull();
      expect(NM_CONTEXTS[name]).toBe(parts.form === "async" || parts.form === "wrap-async" ? "misuse" : "legal");
      expect(name).toBe(`nm-${parts.form}-${parts.host}`);
    }
    for (const other of ["nm-async-body", "nm-async", "pl-ispending-async-tracked", "rs-prop-memo-async"]) expect(nmParts(other), other).toBeNull();
  });
  it("print the lead and its twins: the named function at module scope, handed to isPending from the host's callback", () => {
    const lead = textOf("nm-async-tracked");
    expect(lead).toContain("const cb1 = async () => {");
    expect(lead).toContain("isPending(cb1);");
    expect(lead.indexOf("const cb1 = async () => {")).toBeLessThan(lead.findIndex((line) => line.startsWith("export function C1")));
    expect(lead).toContain("createTrackedEffect(() => {");
    expect(textOf("nm-async-settled")).toContain("onSettled(() => {");
    expect(textOf("nm-wrap-async-tracked")).toContain("isPending(() => cb1());");
    const after = textOf("nm-async-after-tracked");
    expect(after.indexOf("await Promise.resolve();")).toBeGreaterThan(after.indexOf("const cb1 = async () => {"));
    expect(after.indexOf("await Promise.resolve();")).toBeLessThan(after.indexOf("flush();"));
    expect(textOf("nm-sync-tracked")).toContain("const cb1 = () => {");
    expect(textOf("nm-wrap-sync-settled")).toContain("isPending(() => cb1());");
    expect(textOf("nm-gen-tracked").some((line) => line.startsWith("const cb1 = function*"))).toBe(true);
  });
  it("are cells of SPEC on their side, one base shape, not the census's, out of api-fire's plan", () => {
    expect(nmShape("nm-async-tracked", { binding: "alias", layer: "none" })).toBe(false);
    expect(nmShape("pl-ispending-tracked", { binding: "alias", layer: "iife" })).toBe(true);
    for (const context of NM_CONTEXT_NAMES) {
      const side = NM_CONTEXTS[context] === "misuse" ? SPEC[CODE].recallContexts : SPEC[CODE].recallLegal;
      expect(side, context).toContain(context);
      expect((NM_CONTEXTS[context] === "misuse" ? recallShapes : recallLegalShapes)(CODE, context), context).toHaveLength(1);
      expect(BASE_ONLY_CONTEXTS()).toContain(context);
    }
    expect(NM_CODES).toEqual([CODE]);
    expect(censusCells().filter(({ context }) => NM_CONTEXT_NAMES.includes(context))).toEqual([]);
  });
});
