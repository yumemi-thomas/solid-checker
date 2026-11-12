import { describe, expect, it } from "vitest";
import { buildCase } from "./emit.mjs";
import { generateProgram } from "./generate.mjs";
import { recallLegalShapes } from "./legal.mjs";
import { BASE_ONLY_CONTEXTS, SPEC } from "./misuse.mjs";
import { censusCells, recallShapes } from "./matrix.mjs";
import { isValid } from "./model.mjs";
import { PH_CODES, PH_CONTEXTS, PH_CONTEXT_NAMES, PH_HOSTS, PH_PACKAGE, PH_ROUTES, phPackageFiles, phParts, phShape } from "./ph-shapes.mjs";

// The B3 PR after #213: a flush in a function a package the build does not read as source may run, by every route a function reaches the package, with the twins that never run it.

const built = (code, context) => {
  const side = PH_CONTEXTS[context];
  const shape = (side === "misuse" ? recallShapes : recallLegalShapes)(code, context).find((one) => one.binding === "direct" && one.layer === "none");
  const program = generateProgram(7, { profile: "misuse", misusePlan: { code, context, variant: shape.variant, binding: "direct", layer: "none", legal: side === "legal" } });
  const case_ = buildCase(program, { id: `ph-${context}` });
  expect(isValid(program), context).toBe(true);
  expect(case_.neutral, context).toBe(true);
  expect(case_.sites, context).toHaveLength(1);
  return case_;
};
const textOf = (code, context) => built(code, context).text.split("\n").map((line) => line.trim());

describe("the ph contexts (a flush in a function a package nothing here reads may run)", () => {
  it("are eleven routes on three hosts, each with a twin that never runs the site, each name parsed back", () => {
    expect(PH_ROUTES).toEqual(["literal", "ref", "alias", "member", "method", "holder", "array", "jsx", "children", "wrap", "caller"]);
    expect(PH_HOSTS).toEqual(["step", "apply", "leaf"]);
    expect(PH_CONTEXT_NAMES).toHaveLength(66);
    for (const name of PH_CONTEXT_NAMES) {
      const parts = phParts(name);
      expect(parts, name).not.toBeNull();
      expect(PH_CONTEXTS[name]).toBe(parts.never ? "legal" : "misuse");
      expect(name).toBe(`ph-${parts.route}-${parts.host}${parts.never ? "-never" : ""}`);
    }
    for (const other of ["ph-literal", "fc-named-action", "ph-literal-other", "pl-untrack-tracked"]) expect(phParts(other), other).toBeNull();
  });
  it("ship a package that names no Solid package, so the build does not read its source", () => {
    const files = phPackageFiles();
    const manifest = JSON.parse(files["package.json"]);
    expect(manifest.name).toBe(PH_PACKAGE);
    expect(manifest.dependencies).toBeUndefined();
    expect(manifest.peerDependencies).toBeUndefined();
    expect(files["index.js"]).toContain("action(function* ()");
    expect(files["index.js"]).toContain("createEffect(() => 1, () => {");
    expect(files["index.js"]).toContain("createTrackedEffect(() => {");
    for (const name of ["runStep", "runApplyWith", "runLeafAll", "StepView", "ApplyView", "LeafView", "skipRun", "skipRunWith", "skipRunAll", "SkipView"]) expect(files["index.d.ts"], name).toContain(`export declare const ${name}:`);
    const program = built("FLUSH_IN_ACTION", "ph-literal-step");
    expect(Object.keys(program.packages ?? {})).toEqual([PH_PACKAGE]);
  });
  it("print the routes: the site first, then the call that hands the function to the package, or in the twin to a function that ignores it", () => {
    const literal = textOf("FLUSH_IN_ACTION", "ph-literal-step");
    expect(literal).toContain('import { runStep } from "fz-step";');
    expect(literal.some((line) => line.startsWith("runStep(() => {"))).toBe(true);
    const twin = textOf("FLUSH_IN_ACTION", "ph-literal-step-never");
    expect(twin).toContain('import { skipRun } from "fz-step";');
    expect(twin.some((line) => line.startsWith("skipRun(() => {"))).toBe(true);
    expect(textOf("FLUSH_IN_ACTION", "ph-ref-step").some((line) => /^runStep\(cb\d+\);$/.test(line))).toBe(true);
    const alias = textOf("FLUSH_IN_EFFECT_CALLBACK", "ph-alias-apply");
    expect(alias.some((line) => /^const again\d+ = cb\d+;$/.test(line))).toBe(true);
    expect(alias.some((line) => /^runApply\(again\d+\);$/.test(line))).toBe(true);
    expect(textOf("FLUSH_IN_ACTION", "ph-member-step").some((line) => line.startsWith("runStepWith({ run: () => {"))).toBe(true);
    expect(textOf("FLUSH_IN_ACTION", "ph-method-step").some((line) => line.startsWith("runStepWith({ run() {"))).toBe(true);
    const holder = textOf("FLUSH_IN_TRACKED_QUEUE_CALLBACK", "ph-holder-leaf");
    expect(holder.some((line) => line.startsWith("const opts"))).toBe(true);
    expect(holder.some((line) => /^runLeafWith\(opts\d+\);$/.test(line))).toBe(true);
    expect(textOf("FLUSH_IN_ACTION", "ph-array-step").some((line) => line.startsWith("runStepAll([() => {"))).toBe(true);
    expect(textOf("FLUSH_IN_ACTION", "ph-jsx-step").some((line) => line.startsWith("document.body.append((<StepView render={() => {"))).toBe(true);
    expect(textOf("FLUSH_IN_EFFECT_CALLBACK", "ph-children-apply").some((line) => line.startsWith("document.body.append((<ApplyView>{() => {"))).toBe(true);
    expect(textOf("FLUSH_IN_TRACKED_QUEUE_CALLBACK", "ph-jsx-leaf-never").some((line) => line.includes("<SkipView render={() => {"))).toBe(true);
    const wrap = textOf("FLUSH_IN_ACTION", "ph-wrap-step");
    expect(wrap.some((line) => /^function wrap\d+\(run: \(\) => void\) \{$/.test(line))).toBe(true);
    expect(wrap).toContain("runStep(run);");
    const caller = textOf("FLUSH_IN_ACTION", "ph-caller-step");
    expect(caller.some((line) => /^function helper\d+\(\) \{$/.test(line))).toBe(true);
    expect(caller.some((line) => /^runStep\(\(\) => helper\d+\(\)\);$/.test(line))).toBe(true);
  });
  it("are cells of SPEC on their side for the codes their host reports, one base shape, not the census's, out of api-fire's plan", () => {
    expect(phShape("ph-literal-step", { binding: "alias", layer: "none" })).toBe(false);
    expect(phShape("pl-untrack-tracked", { binding: "alias", layer: "iife" })).toBe(true);
    for (const [code, hosts] of Object.entries(PH_CODES)) {
      for (const context of PH_CONTEXT_NAMES.filter((name) => hosts.includes(phParts(name).host))) {
        const side = PH_CONTEXTS[context] === "misuse" ? SPEC[code].recallContexts : SPEC[code].recallLegal;
        expect(side, `${code} ${context}`).toContain(context);
        const shapes = (PH_CONTEXTS[context] === "misuse" ? recallShapes : recallLegalShapes)(code, context);
        expect(shapes.length, `${code} ${context}`).toBeGreaterThan(0);
        for (const shape of shapes) expect({ binding: shape.binding, layer: shape.layer ?? "none" }, `${code} ${context}`).toEqual({ binding: "direct", layer: "none" });
      }
    }
    for (const context of PH_CONTEXT_NAMES) expect(BASE_ONLY_CONTEXTS()).toContain(context);
    expect(Object.keys(PH_CODES)).toEqual(["FLUSH_IN_EFFECT_CALLBACK", "FLUSH_IN_ACTION", "FLUSH_IN_TRACKED_QUEUE_CALLBACK"]);
    expect(censusCells().filter(({ context }) => PH_CONTEXT_NAMES.includes(context))).toEqual([]);
  });
});
