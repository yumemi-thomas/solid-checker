import { describe, expect, it } from "vitest";
import { buildCase } from "./emit.mjs";
import { generateProgram } from "./generate.mjs";
import { recallLegalShapes } from "./legal.mjs";
import { BASE_ONLY_CONTEXTS, SPEC } from "./misuse.mjs";
import { recallShapes } from "./matrix.mjs";
import { isValid } from "./model.mjs";
import { PA_CODE, PA_CONTEXTS, PA_CONTEXT_NAMES, PA_PACKAGES, paPackageFiles, paParts } from "./pa-shapes.mjs";

// ADR 0972: a call into code of a package the build does not read, and an action called where it is created, on owned and legal hosts.

const built = (context) => {
  const side = PA_CONTEXTS[context];
  const shape = (side === "misuse" ? recallShapes : recallLegalShapes)(PA_CODE, context).find((one) => one.binding === "direct" && one.layer === "none");
  const program = generateProgram(7, { profile: "misuse", misusePlan: { code: PA_CODE, context, variant: shape.variant, binding: "direct", layer: "none", legal: side === "legal" } });
  const case_ = buildCase(program, { id: `pa-${context}` });
  expect(isValid(program), context).toBe(true);
  expect(case_.sites, context).toHaveLength(1);
  return case_;
};

describe("the pa contexts (a package the build does not read may call an action)", () => {
  it("are four package kinds on five hosts, misuse where the call throws, each name parsed back", () => {
    expect(PA_CONTEXT_NAMES).toHaveLength(20);
    for (const name of PA_CONTEXT_NAMES) {
      const parts = paParts(name);
      expect(parts, name).not.toBeNull();
      const throws = ["reach", "inline"].includes(parts.kind) && ["body", "memo", "root"].includes(parts.host);
      expect(PA_CONTEXTS[name], name).toBe(throws ? "misuse" : "legal");
    }
    expect(paParts("pa-reach")).toBeNull();
    expect(paParts("ph-literal-step")).toBeNull();
  });
  it("ship packages that name no Solid package, so the build does not read their source", () => {
    for (const kind of Object.keys(PA_PACKAGES)) {
      const manifest = JSON.parse(paPackageFiles(kind)["package.json"]);
      expect(manifest.name).toBe(PA_PACKAGES[kind]);
      expect(manifest.dependencies ?? {}).toEqual({});
      expect(manifest.peerDependencies ?? {}).toEqual({});
    }
    expect(paPackageFiles("reach")["index.js"]).toContain('from "solid-js"');
    expect(paPackageFiles("pure")["index.js"]).not.toContain("import");
    expect(paPackageFiles("dyn")["index.js"]).toContain("import(name)");
  });
  it("are recall contexts of ACTION_CALLED_IN_OWNED_SCOPE only, base shape only", () => {
    expect(SPEC[PA_CODE].recallContexts.filter((context) => context.startsWith("pa-")).sort()).toEqual(PA_CONTEXT_NAMES.filter((name) => PA_CONTEXTS[name] === "misuse").sort());
    expect(SPEC.ON_FLUSH_IN_OWNED_SCOPE.recallContexts.some((context) => context.startsWith("pa-"))).toBe(false);
    for (const name of PA_CONTEXT_NAMES) expect(BASE_ONLY_CONTEXTS()).toContain(name);
  });
  it("write the site as the package's call or the inline action, on its host", () => {
    const text = (context) => built(context).text;
    expect(text("pa-reach-body")).toContain('import { paEnter } from "fz-act";');
    expect(text("pa-reach-body")).toMatch(/\n {2}(__h\(\d+\); )?paEnter\(\);/);
    expect(text("pa-inline-memo")).toMatch(/createMemo\(\(\) => \{\n\s+(__h\(\d+\); )?action\(function\* \(\) \{\}\)\(\);/);
    expect(text("pa-pure-timer")).toContain('import { paEnter } from "fz-calm";');
    expect(text("pa-dyn-root")).toContain("createRoot(");
    expect(Object.keys(built("pa-reach-root").files)).toContain("node_modules/fz-act/index.js");
  });
});
