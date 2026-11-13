import { describe, expect, it } from "vitest";
import { buildCase } from "./emit.mjs";
import { generateProgram } from "./generate.mjs";
import { recallLegalShapes } from "./legal.mjs";
import { BASE_ONLY_CONTEXTS, SPEC } from "./misuse.mjs";
import { censusCells, recallShapes } from "./matrix.mjs";
import { isValid } from "./model.mjs";
import { ER_CODES, ER_CONTEXTS, ER_CONTEXT_NAMES, ER_HOSTS, ER_ROUTES, erParts, erShape } from "./er-shapes.mjs";

// Batch 11, slice 6 (the review of #220, F1-F3): a function the host's callback reaches by a reference, as generator cells with their twins.

const textOf = (code, context, variant) => {
  const side = ER_CONTEXTS[context];
  const shape = (side === "misuse" ? recallShapes : recallLegalShapes)(code, context).find((one) => one.binding === "direct" && one.layer === "none" && (variant === undefined || one.variant === variant));
  expect(shape, `${context} ${variant}`).toBeDefined();
  const program = generateProgram(7, { profile: "misuse", misusePlan: { code, context, variant: shape.variant, binding: "direct", layer: "none", legal: side === "legal" } });
  const built = buildCase(program, { id: `er-${context}` });
  expect(isValid(program), context).toBe(true);
  expect(built.neutral, context).toBe(true);
  expect(built.sites, context).toHaveLength(1);
  return built.text.split("\n").map((line) => line.trim());
};

describe("the er contexts (a function reached by a reference)", () => {
  it("are twenty-two routes on an apply and the view route on the two leaf hosts, each with a twin that never runs the site, each name parsed back", () => {
    expect(ER_ROUTES).toEqual(["view", "array", "saved", "timer", "generator", "root", "genlayer", "genuntrack", "genhelper", "rootself", "prop", "slot", "children", "slotcall", "childcall", "slothelper", "childhelper", "rownest", "shownest", "getteriife", "gettermap", "providernest"]);
    expect(ER_HOSTS).toEqual(["apply", "tracked", "settled"]);
    expect(ER_CONTEXT_NAMES).toHaveLength(2 * (22 + 2));
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
  it("print the generator-layer routes: the site in an arrow or an untrack callback the generator's body runs, stepped by for...of or spread (the twin only keeps the iterator)", () => {
    const layer = textOf("PRIMITIVE_IN_EFFECT_CALLBACK", "er-genlayer-apply");
    const at = layer.findIndex((line) => line.startsWith("function* g1() { const f1 = () => {"));
    expect(at).toBeGreaterThan(-1);
    expect(layer.slice(at).join(" ")).toContain("}; f1(); yield 1; }");
    expect(layer).toContain("const it1 = g1();");
    expect(layer.join("\n")).toContain("for (const _ of it1) { void _; }");
    const layerTwin = textOf("PRIMITIVE_IN_EFFECT_CALLBACK", "er-genlayer-apply-never");
    expect(layerTwin).toContain("void it1;");
    expect(layerTwin.join("\n")).not.toContain("for (const _ of");
    const untracked = textOf("PRIMITIVE_IN_EFFECT_CALLBACK", "er-genuntrack-apply");
    expect(untracked.findIndex((line) => line.startsWith("function* g1() { untrack(() => {"))).toBeGreaterThan(-1);
    expect(untracked.join(" ")).toContain("}); yield 1; }");
    expect(untracked).toContain("void [...it1];");
    const untrackedTwin = textOf("PRIMITIVE_IN_EFFECT_CALLBACK", "er-genuntrack-apply-never");
    expect(untrackedTwin).toContain("void it1;");
    expect(untrackedTwin.join("\n")).not.toContain("[...it1]");
  });
  it("print the genhelper route: a function written beside the generator and called from its body, the iterator made at module scope and stepped by the apply (the twin only keeps it)", () => {
    const lead = textOf("PRIMITIVE_IN_EFFECT_CALLBACK", "er-genhelper-apply");
    expect(lead).toContain("let it1: Generator<number> | undefined;");
    expect(lead).toContain("createRoot(() => {");
    expect(lead.join("\n")).toContain("function hh1() {");
    expect(lead).toContain("function* g1() { hh1(); yield 1; }");
    expect(lead).toContain("it1 = g1();");
    expect(lead.join("\n")).toContain("for (const _ of it1!) { void _; }");
    const twin = textOf("PRIMITIVE_IN_EFFECT_CALLBACK", "er-genhelper-apply-never");
    expect(twin).toContain("void it1;");
    expect(twin.join("\n")).not.toContain("for (const _ of");
  });
  it("print the rootself route: a createRoot callback that is a named function expression saving itself, called by the apply (the twin only keeps the holder)", () => {
    const lead = textOf("PRIMITIVE_IN_EFFECT_CALLBACK", "er-rootself-apply");
    expect(lead).toContain("let sv1: (() => void) | undefined;");
    expect(lead).toContain("createRoot(function f1() {");
    expect(lead).toContain("if (!sv1) { sv1 = f1; return; };");
    expect(lead).toContain("sv1!();");
    const twin = textOf("PRIMITIVE_IN_EFFECT_CALLBACK", "er-rootself-apply-never");
    expect(twin).toContain("void sv1;");
    expect(twin).not.toContain("sv1!();");
  });
  it("print the prop route: an arrow written in a component element's prop that the child's apply calls (the twin's child never calls it)", () => {
    const lead = textOf("PRIMITIVE_IN_EFFECT_CALLBACK", "er-prop-apply", "jsx-insert");
    const at = lead.findIndex((line) => line.startsWith("<Kid1 run={() => {"));
    expect(at).toBeGreaterThan(-1);
    expect(lead.slice(at, at + 3).join(" ")).toContain("<p>{String(Math.random())}</p>;");
    expect(lead).toContain("function Kid1(props: { run: () => void }) {");
    expect(lead).toContain("createEffect(() => 1, () => { props.run(); });");
    const twin = textOf("PRIMITIVE_IN_EFFECT_CALLBACK", "er-prop-apply-never", "jsx-insert");
    expect(twin.some((line) => line.startsWith("<Kid1 run={() => {"))).toBe(true);
    expect(twin.join("\n")).not.toContain("props.run()");
  });
  it("print the slot and children routes: a getter the child's apply reads, a prop's value or the element's children (the twin's child reads nothing)", () => {
    const kid = (read) => `const KidG1 = (props: { slot?: unknown; children?: unknown }) => { createEffect(() => 1, () => { ${read}; }); return <p>k</p>; };`;
    const slot = textOf("PRIMITIVE_IN_EFFECT_CALLBACK", "er-slot-apply", "jsx-insert");
    expect(slot).toContain(kid("void props.slot"));
    expect(slot).toContain("<KidG1 slot={<p>{String(Math.random())}</p>} />");
    const children = textOf("PRIMITIVE_IN_EFFECT_CALLBACK", "er-children-apply", "jsx-component");
    expect(children).toContain(kid("void props.children"));
    expect(children).toContain("<KidG1>{<JsxComp1 />}</KidG1>");
    for (const [route, element] of [["slot", "<KidG1 slot={<p>{String(Math.random())}</p>} />"], ["children", "<KidG1>{<p>{String(Math.random())}</p>}</KidG1>"]]) {
      const twin = textOf("PRIMITIVE_IN_EFFECT_CALLBACK", `er-${route}-apply-never`, "jsx-insert");
      expect(twin, route).toContain(kid("void props"));
      expect(twin, route).toContain(element);
    }
    // A creating call is a getter's expression the same way.
    expect(textOf("PRIMITIVE_IN_EFFECT_CALLBACK", "er-slot-apply", "memo").join("\n")).toMatch(/<KidG1 slot=\{createMemo\(\(\) => 1\)\} \/>/);
  });
  it("print the slotcall and childcall routes: the site in an arrow the getter's own evaluation calls, at module scope in a root (the twin's child reads nothing)", () => {
    const slot = textOf("PRIMITIVE_IN_EFFECT_CALLBACK", "er-slotcall-apply", "jsx-insert");
    const at = slot.findIndex((line) => line.startsWith("createRoot(() => { void (<KidG1 slot={(() => {"));
    expect(at).toBeGreaterThan(-1);
    expect(slot.slice(at, at + 4).join(" ")).toContain("<p>{String(Math.random())}</p>; return 1; })()} />); });");
    expect(slot.join("\n")).toContain("void props.slot;");
    // Module scope: no function of the component around the creation (B3 files a row at every site written inside an exported component).
    expect(slot.findIndex((line) => line.startsWith("export function C1"))).toBeGreaterThan(at);
    const children = textOf("PRIMITIVE_IN_EFFECT_CALLBACK", "er-childcall-apply", "memo");
    expect(children.findIndex((line) => line.startsWith("createRoot(() => { void (<KidG1>{(() => {"))).toBeGreaterThan(-1);
    expect(children.join("\n")).toContain("void props.children;");
    expect(children.join("\n")).toContain("createMemo(() => 1);");
    for (const route of ["slotcall", "childcall"]) {
      const twin = textOf("PRIMITIVE_IN_EFFECT_CALLBACK", `er-${route}-apply-never`, "jsx-insert").join("\n");
      expect(twin, route).toContain("{ void props; }");
      expect(twin, route).toContain("})()");
    }
  });
  it("print the slothelper and childhelper routes: a function at module scope holds the site and the getter calls it, in a module-level root (the twin's child reads nothing)", () => {
    const slot = textOf("PRIMITIVE_IN_EFFECT_CALLBACK", "er-slothelper-apply", "jsx-insert");
    const at = slot.findIndex((line) => /^function h\d+\(\) \{$/.test(line));
    expect(at).toBeGreaterThan(-1);
    expect(slot.slice(at, at + 4).join(" ")).toContain("<p>{String(Math.random())}</p>; return 1; }");
    expect(slot.some((line) => /^createRoot\(\(\) => \{ void \(<KidG1 slot=\{h\d+\(\)\} \/>\); \}\);$/.test(line))).toBe(true);
    expect(slot.join("\n")).toContain("void props.slot;");
    const children = textOf("PRIMITIVE_IN_EFFECT_CALLBACK", "er-childhelper-apply", "memo");
    expect(children.some((line) => /^createRoot\(\(\) => \{ void \(<KidG1>\{h\d+\(\)\}<\/KidG1>\); \}\);$/.test(line))).toBe(true);
    expect(children.join("\n")).toContain("void props.children;");
    for (const route of ["slothelper", "childhelper"]) {
      const twin = textOf("PRIMITIVE_IN_EFFECT_CALLBACK", `er-${route}-apply-never`, "memo").join("\n");
      expect(twin, route).toContain("{ void props; }");
      expect(twin, route).toMatch(/h\d+\(\)/);
    }
  });
  it("print the nested in-place routes: an arrow stored by code written inside the function a dialect element is handed, or inside its in-place getter, at module scope in a root (the twin keeps the holder only)", () => {
    const row = textOf("PRIMITIVE_IN_EFFECT_CALLBACK", "er-rownest-apply", "jsx-insert");
    expect(row).toContain("let sv1: (() => void) | undefined;");
    const at = row.findIndex((line) => line.startsWith("function Rows1(_props: { a?: unknown }) { return <p><For each={[1]}>{() => { sv1 = () => {"));
    expect(at).toBeGreaterThan(-1);
    expect(row.slice(at, at + 4).join(" ")).toContain("<p>{String(Math.random())}</p>; }; return <p>r</p>; }}</For></p>; }");
    expect(row).toContain("sv1!();");
    expect(row.join("\n")).toContain("<Rows1 a={1} />");
    // Module code: no function of the exported component around the creation (B3 files a row at every site written inside an exported component).
    expect(row.findIndex((line) => line.startsWith("export function C1"))).toBeGreaterThan(at);
    const show = textOf("PRIMITIVE_IN_EFFECT_CALLBACK", "er-shownest-apply", "memo");
    expect(show.findIndex((line) => line.startsWith("createRoot(() => { void (<Show when={1}>{(v) => { void v; sv1 = () => {"))).toBeGreaterThan(-1);
    expect(show.join(" ")).toContain("createMemo(() => 1); }; return <p>s</p>; }}</Show>); });");
    const iife = textOf("PRIMITIVE_IN_EFFECT_CALLBACK", "er-getteriife-apply", "jsx-component");
    expect(iife.findIndex((line) => line.startsWith("createRoot(() => { void (<Show when={1}>{(() => { sv1 = () => {"))).toBeGreaterThan(-1);
    expect(iife.join(" ")).toContain("<JsxComp1 />; }; return <p>i</p>; })()}</Show>); });");
    const map = textOf("PRIMITIVE_IN_EFFECT_CALLBACK", "er-gettermap-apply", "jsx-attr");
    expect(map.findIndex((line) => line.startsWith("createRoot(() => { void (<Show when={1}>{[1].map(() => { sv1 = () => {"))).toBeGreaterThan(-1);
    expect(map.join(" ")).toContain("}; return <p>m</p>; })}</Show>); });");
    const provider = textOf("PRIMITIVE_IN_EFFECT_CALLBACK", "er-providernest-apply", "jsx-insert");
    expect(provider).toContain("const Ctx1 = createContext(1);");
    expect(provider.findIndex((line) => line.startsWith("function Rows1(_props: { a?: unknown }) { return <p><Ctx1 value={1}>{(() => { sv1 = () => {"))).toBeGreaterThan(-1);
    expect(provider.join(" ")).toContain("}; return <p>c</p>; })()}</Ctx1></p>; }");
    expect(provider.join("\n")).toContain("<Rows1 a={1} />");
    for (const route of ["rownest", "shownest", "getteriife", "gettermap", "providernest"]) {
      const twin = textOf("PRIMITIVE_IN_EFFECT_CALLBACK", `er-${route}-apply-never`, "jsx-insert").join("\n");
      expect(twin, route).toContain("void sv1;");
      expect(twin, route).not.toContain("sv1!()");
      expect(twin, route).toContain("sv1 = () => {");
    }
  });
  it("take the JSX variants of PRIMITIVE_IN_EFFECT_CALLBACK in the view, prop, slot, children, slotcall, childcall, slothelper, childhelper and nested in-place cells only (a JSX creation is a site of the universe, ADR 0971)", () => {
    const jsx = ["jsx-insert", "jsx-attr", "jsx-spread", "jsx-fragment", "jsx-component"];
    for (const route of ["view", "prop", "slot", "children", "slotcall", "childcall", "slothelper", "childhelper", "rownest", "shownest", "getteriife", "gettermap", "providernest"]) {
      const variants = recallShapes("PRIMITIVE_IN_EFFECT_CALLBACK", `er-${route}-apply`).map((shape) => shape.variant);
      for (const variant of jsx) expect(variants, `${route} ${variant}`).toContain(variant);
      expect(recallLegalShapes("PRIMITIVE_IN_EFFECT_CALLBACK", `er-${route}-apply-never`).map((shape) => shape.variant), route).toEqual(expect.arrayContaining(jsx));
    }
    for (const route of ["array", "saved", "timer", "generator", "root"]) {
      const variants = recallShapes("PRIMITIVE_IN_EFFECT_CALLBACK", `er-${route}-apply`).map((shape) => shape.variant);
      for (const variant of jsx) expect(variants, `${route} ${variant}`).not.toContain(variant);
    }
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
