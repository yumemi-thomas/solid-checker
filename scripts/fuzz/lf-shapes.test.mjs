import { describe, expect, it } from "vitest";
import { buildCase } from "./emit.mjs";
import { generateProgram } from "./generate.mjs";
import { recallLegalShapes } from "./legal.mjs";
import { BASE_ONLY_CONTEXTS, SPEC } from "./misuse.mjs";
import { recallShapes } from "./matrix.mjs";
import { isValid } from "./model.mjs";
import { LF_ACCESSOR, LF_CONTEXTS, LF_CONTEXT_NAMES, LF_COVER, LF_FORMS, LF_HOLD, lfParts } from "./lf-shapes.mjs";

// ADR 0978: `Loading` entered as a call or a value, what covers a read under a hand-called or call-form boundary, and the `fallback` read.

const CODE_OF = { hold: LF_HOLD, acc: LF_ACCESSOR, cover: LF_COVER, fb: LF_COVER };
const built = (context) => {
  const { kind } = lfParts(context);
  const code = CODE_OF[kind];
  const side = LF_CONTEXTS[context];
  const shape = (side === "misuse" ? recallShapes : recallLegalShapes)(code, context).find((one) => one.binding === "direct" && one.layer === "none");
  const program = generateProgram(7, { profile: "misuse", misusePlan: { code, context, variant: shape.variant, binding: "direct", layer: "none", legal: side === "legal" } });
  expect(isValid(program), context).toBe(true);
  return buildCase(program, { id: `lf-${context}` });
};

describe("the lf contexts (Loading entered other than as a JSX tag)", () => {
  it("are named by kind and form, each parsed back, misuse or legal by what Chrome does", () => {
    expect(LF_CONTEXT_NAMES).toHaveLength(32);
    for (const name of LF_CONTEXT_NAMES) {
      const parts = lfParts(name);
      expect(parts, name).not.toBeNull();
      expect(Object.keys(LF_FORMS), name).toContain(parts.form);
      expect(LF_CONTEXTS[name], name).toBe(parts.kind === "cover" ? "legal" : "misuse");
    }
    expect(lfParts("loading-on-outside")).toBeNull();
    expect(lfParts("lf-cover-dynamic-el")).toBeNull();
  });
  it("are recall contexts of the three codes they ask, base shape only", () => {
    for (const name of LF_CONTEXT_NAMES) {
      const code = CODE_OF[lfParts(name).kind];
      const cells = LF_CONTEXTS[name] === "legal" ? [...(SPEC[code].legal ?? []), ...(SPEC[code].recallLegal ?? [])] : [...SPEC[code].contexts, ...(SPEC[code].recallContexts ?? [])];
      expect(cells, name).toContain(name);
      expect(BASE_ONLY_CONTEXTS(), name).toContain(name);
    }
  });
  it("write the hold cell's boundary as a call with getters, the sibling read outside it", () => {
    const text = built("lf-hand").text;
    expect(text).toMatch(/\{Loading\(\{ get on\(\) \{ return t\d\(\); \}, get fallback\(\) \{ return <p>f<\/p>; \}, get children\(\) \{ return <><p data-view="v\d">\{a\d\(\)\}<\/p><\/>; \} \}\)\}/);
    expect(text).toContain("createMemo");
    expect(built("lf-createcomponent").text).toContain("createComponent(Loading, {");
    expect(built("lf-dotcall").text).toContain("Loading.call(null, {");
    expect(built("lf-array").text).toContain("[Loading][0]({");
    expect(built("lf-alias-hand").text).toContain("import { Loading as Hold,");
    expect(built("lf-ns-hand").text).toContain('import * as S from "solid-js";');
    expect(built("lf-wrapper-hand").text).toContain("function LfWrap(p: any) {");
    expect(built("lf-dynamic").text).toContain("const LfDyn: any = dynamicComponent(() => Loading);");
    expect(built("lf-dynamic-el").text).toContain("<Dynamic component={Loading}");
    expect(built("lf-unresolved-param").text).toContain("<p.as on={p.on}");
  });
  it("write the accessor cell's `on` as a data property, the cover cell's read in the children getter, the fallback cell's read in the fallback", () => {
    expect(built("lf-acc-createcomponent").text).toMatch(/get children\(\)/);
    expect(built("lf-acc-dotcall").sites).toBeDefined();
    const cover = built("lf-cover-hand");
    expect(cover.sites).toHaveLength(1);
    expect(cover.text).toMatch(/get children\(\) \{ return <><p>\{a\d\(\)\}<\/p><\/>; \}/);
    expect(cover.text).not.toContain("on()");
    const fallback = built("lf-fb-jsx");
    expect(fallback.text).toMatch(/<Loading fallback=\{<><p>\{a\d\(\)\}<\/p><\/>\}>/);
    expect(built("lf-fb-clb").text).toContain("createLoadingBoundary(() => <>");
  });
});
