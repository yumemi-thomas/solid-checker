// Run: bun packages/cli/node_modules/vitest/vitest.mjs run --config packages/cli/vitest.config.mjs scripts/fuzz/package-site.test.mjs
// ADR 0957: the package site (package-site.mjs): a program's site moved into a dependency package and reached through an entry, for every source and context.
import { describe, expect, it } from "vitest";
import { buildCase } from "./emit.mjs";
import { generateProgram } from "./generate.mjs";
import { stripInstrumentation } from "./hits.mjs";
import { cellList, componentSide, plainShape, recallShapes, shapeId } from "./matrix.mjs";
import { legalCells, legalId, recallLegalShapes } from "./legal.mjs";
import { MODULE, SOURCES, SPEC, parentSource, sourcesOf } from "./misuse.mjs";
import { validate, walk } from "./model.mjs";
import { COMPONENT_ENTRIES, PKG_ENTRIES, freeNames, packageSite, parsePkg, pkgShapesOf } from "./package-site.mjs";
import { packageCells } from "./package-cells.mjs";
import { PACKAGE_SITE_EXEMPT_CONTEXTS, PACKAGE_SITE_EXEMPT_SOURCES, packageSiteOf } from "./package-site-coverage.mjs";

const program = (code, context, extra = {}) => generateProgram(7, { profile: "misuse", misusePlan: { code, context, binding: "direct", layer: "none", variant: null, census: true, ...extra } });
const base = (over = {}) => ({ text: "setW1(7)", solid: [], web: [], namespaces: [], moduleLines: [], preludeLines: [], contextLine: undefined, importLines: undefined, files: undefined, ...over });

describe("freeNames", () => {
  it("lists the names a text uses and does not declare, in order of appearance", () => {
    expect(freeNames("setW1(7)")).toEqual(["setW1"]);
    expect(freeNames("w1(); setW1(w1() + 1);")).toEqual(["w1", "setW1"]);
    expect(freeNames("props.w1()")).toEqual(["props"]);
  });
  it("leaves out declarations, property names, intrinsic tags and globals", () => {
    expect(freeNames("const t = {}; t.x = a; Math.max(1, b); Reflect.get(t, 'x'); queueMicrotask(() => c())")).toEqual(["a", "b", "c"]);
    expect(freeNames("const f = (x) => x + d; f(1)")).toEqual(["d"]);
    expect(freeNames("<p class={e}>x</p>")).toEqual(["e"]);
    expect(freeNames("<JsxComp1 />", ["JsxComp1"])).toEqual([]);
    expect(freeNames("<JsxComp1 />")).toEqual(["JsxComp1"]);
    expect(freeNames("({ k: v, w })")).toEqual(["v", "w"]);
  });
  it("takes the names a caller binds as declared", () => {
    expect(freeNames("untrack(() => w1())", ["untrack"])).toEqual(["w1"]);
  });
});

describe("packageSite", () => {
  it("writes each entry's call and export", () => {
    const expected = {
      call: ["site1(setW1)", /export function site1\(setW1\) \{\n  setW1\(7\);\n\}/],
      "call-inner": ["site1(setW1)", /function inner1\(setW1\)[\s\S]*export function site1\(setW1\) \{\n  return inner1\(setW1\);/],
      dotcall: ["site1.call(null, setW1)", /export function site1\(setW1\)/],
      reflect: ["Reflect.apply(site1, undefined, [setW1])", /export function site1\(setW1\)/],
      tagged: ["site1`${setW1}`", /export function site1\(strings, setW1\)/],
      new: ["new Site1(setW1)", /export class Site1 \{\n  constructor\(setW1\) \{\n    setW1\(7\);/],
      getter: ["make1(setW1).value", /export function make1\(setW1\)[\s\S]*get value\(\) \{\n\s+setW1\(7\);\n\s+return 1;/],
      callback: ["apply1(site1, setW1)", /export function site1\(setW1\)/],
      jsx: ["<Site1 setW1={setW1} />", /export function Site1\(__p\) \{\n  const \{ setW1 \} = __p;\n  setW1\(7\);\n  return null;/],
      "jsx-inner": ["<Site1 setW1={setW1} />", /function Inner1\(__p\)[\s\S]*return createComponent\(Inner1, __p\);/],
      hop: ["createComponent(Site1, { setW1 })", /export function Site1\(__p\)/],
      dynamic: ["render(() => <SiteDyn1 setW1={setW1} />, document.createElement(\"div\"))", /export function Site1\(__p\)/],
      lazy: ["render(() => <SiteLazy1 setW1={setW1} />, document.createElement(\"div\"))", /export default Site1;/],
    };
    expect(Object.keys(expected).sort()).toEqual([...PKG_ENTRIES].sort());
    for (const entry of PKG_ENTRIES) {
      const moved = packageSite(base(), entry, MODULE);
      expect(moved.text, entry).toBe(expected[entry][0]);
      expect(moved.pkgFiles["fz-site"]["index.js"], entry).toMatch(expected[entry][1]);
      expect(moved.pkgFiles["fz-site"]["package.json"], entry).toContain('"name": "fz-site"');
      expect(moved.pkgFiles["fz-site"]["index.d.ts"], entry).toContain("export declare");
    }
  });
  it("imports Solid's names in the package and hands it the values only", () => {
    const moved = packageSite(base({ text: "flush(() => w1())", solid: ["flush"] }), "call", MODULE);
    expect(moved.text).toBe("site1(w1)");
    expect(moved.pkgFiles["fz-site"]["index.js"]).toContain('import { flush } from "solid-js";');
    // A name no specifier brought but Solid exports (`createContext`, which the printer imports for a `contextLine`): imported, never a parameter.
    const context = packageSite(base({ text: "useContext(Ctx)", solid: ["useContext"], contextLine: "const Ctx = createContext<number>();" }), "call", MODULE);
    expect(context.text).toBe("site1()");
    expect(context.pkgFiles["fz-site"]["index.js"]).toContain('import { createContext, useContext } from "solid-js";');
    expect(context.pkgFiles["fz-site"]["index.js"]).toMatch(/const Ctx = createContext\s*\(\);/);
  });
  it("keeps a project module the site's binding imports in the project, and hands the package the names it binds", () => {
    const moved = packageSite(base({ text: "createEffectN(() => 1, () => {})", importLines: ['import { createEffect as createEffectN } from "./solid-api";'], files: { "solid-api.ts": "x" } }), "call", MODULE);
    expect(moved.text).toBe("site1(createEffectN)");
    expect(moved.importLines).toEqual(['import { site1 } from "fz-site";', 'import { createEffect as createEffectN } from "./solid-api";']);
    expect(moved.files).toEqual({ "solid-api.ts": "x" });
  });
  it("never writes a type annotation into the package", () => {
    const moved = packageSite(base({ text: "(onSettled as any)(() => () => {})", solid: ["onSettled"] }), "call", MODULE);
    expect(moved.pkgFiles["fz-site"]["index.js"]).not.toMatch(/\bas any\b|: any/);
  });
});

describe("a program with its site in a package", () => {
  for (const entry of PKG_ENTRIES) {
    it(`prints the ${entry} entry in the project and the site in the package`, () => {
      const p = program("REACTIVE_WRITE_IN_OWNED_SCOPE", "body", { variant: "signal", pkg: entry });
      expect(validate(p)).toEqual([]);
      const built = buildCase(p);
      expect(built.text).not.toContain("setW1(7)");
      expect(built.packages["fz-site"]["index.js"]).toContain("setW1(7)");
      expect(built.sites).toHaveLength(1);
      expect(Object.keys(built.files).filter((name) => name.startsWith("node_modules/fz-site/")).sort()).toEqual(["node_modules/fz-site/index.d.ts", "node_modules/fz-site/index.js", "node_modules/fz-site/package.json"]);
      expect(JSON.parse(built.files["oracle.json"]).packages).toEqual(["fz-site"]);
      expect(stripInstrumentation(built.runText)).toBe(built.text);
      expect(built.neutral).toBe(true);
      const sites = [...walk(p)].filter(({ node }) => node.k === "use");
      expect(sites).toHaveLength(1);
      expect(sites[0].node.pkg).toEqual({ entry });
    });
  }
  // The own mode: the package creates the signals the site names, so the entry hands it nothing and the checker has the access's text to prove.
  it("creates the signals itself in the own mode, and hands the entry nothing", () => {
    for (const [variant, declaration] of [["signal", /const \[w1, setW1\] = createSignal\(0\);/], ["store", /const \[s1, setS1\] = createStore\(\{ v: 0 \}\);/]]) {
      const code = variant === "store" ? "STORE_WRITE_OUTSIDE_SETTER" : "REACTIVE_WRITE_IN_OWNED_SCOPE";
      const p = program(code, "body", { variant, pkg: "tagged-own" });
      expect(validate(p)).toEqual([]);
      const built = buildCase(p);
      expect(built.text).toContain("site1`x`");
      expect(built.packages["fz-site"]["index.js"]).toMatch(declaration);
      expect(built.packages["fz-site"]["index.js"]).toMatch(/export function site1\(strings\)/);
    }
    expect(pkgShapesOf([])).toEqual(PKG_ENTRIES);
    expect(pkgShapesOf([{ name: "w1", setter: "setW1" }])).toEqual(PKG_ENTRIES.flatMap((entry) => [entry, `${entry}-own`]));
    expect(pkgShapesOf([{ name: "a1", setter: "setA1", kind: "async" }])).toEqual(PKG_ENTRIES);
    expect(parsePkg("jsx-inner-own")).toEqual({ entry: "jsx-inner", own: true });
    expect(parsePkg("call")).toEqual({ entry: "call", own: false });
  });
  it("takes the direct binding only, and an entry it knows", () => {
    expect(() => program("REACTIVE_WRITE_IN_OWNED_SCOPE", "body", { variant: "signal", binding: "alias", pkg: "call" })).toThrow(/direct binding/);
    expect(() => program("REACTIVE_WRITE_IN_OWNED_SCOPE", "body", { variant: "signal", pkg: "nowhere" })).toThrow(/unknown package entry/);
  });
  it("is the same program, byte for byte, when asked twice", () => {
    expect(buildCase(program("STRICT_READ_UNTRACKED", "body", { variant: "signal", pkg: "tagged" })).files).toEqual(buildCase(program("STRICT_READ_UNTRACKED", "body", { variant: "signal", pkg: "tagged" })).files);
  });
});

describe("the package site's grid shapes", () => {
  it("cross the plain shape of each cell with every entry, and name them", () => {
    const shapes = recallShapes("CONTEXT_NOT_FOUND", "body").filter((shape) => shape.pkg);
    expect(shapes.map((shape) => shape.pkg).sort()).toEqual([...PKG_ENTRIES].sort());
    // A template with signals takes each entry in both value modes.
    const writes = recallShapes("REACTIVE_WRITE_IN_OWNED_SCOPE", "body").filter((shape) => shape.pkg && shape.variant === "signal");
    expect(writes.map((shape) => shape.pkg).sort()).toEqual(PKG_ENTRIES.flatMap((entry) => [entry, `${entry}-own`]).sort());
    for (const shape of shapes) expect(plainShape({ ...shape, pkg: undefined })).toBe(true);
    const cell = cellList({ codes: ["CONTEXT_NOT_FOUND"], recall: true }).find((one) => one.context === "body");
    expect(shapes.map((shape) => shapeId(cell, shape))).toContain("mx-context-not-found-body-x-direct-none-pk-tagged");
    const legalCell = legalCells({ codes: ["REACTIVE_WRITE_IN_OWNED_SCOPE"], recall: true }).find((one) => one.context === "handler");
    const legal = recallLegalShapes("REACTIVE_WRITE_IN_OWNED_SCOPE", "handler").filter((shape) => shape.pkg);
    expect(legal.length).toBeGreaterThan(0);
    expect(legalId(legalCell, { shape: legal[0] })).toMatch(/-pk-/);
    // A component entry runs the site in the component's own body: the code's `body` cell decides which side it is on, never the cell's context.
    expect(componentSide("REACTIVE_WRITE_IN_OWNED_SCOPE")).toBe("misuse");
    expect(componentSide("NO_OWNER_EFFECT")).toBe("legal");
    expect(legal.some((shape) => COMPONENT_ENTRIES.includes(shape.pkg))).toBe(false);
    expect(recallShapes("REACTIVE_WRITE_IN_OWNED_SCOPE", "handler").filter((shape) => shape.pkg)).toEqual([]);
    expect(recallShapes("NO_OWNER_EFFECT", "handler").filter((shape) => COMPONENT_ENTRIES.includes(shape.pkg))).toEqual([]);
    expect([...new Set(recallLegalShapes("NO_OWNER_EFFECT", "body").filter((shape) => COMPONENT_ENTRIES.includes(shape.pkg)).map((shape) => shape.pkg))].sort()).toEqual([...COMPONENT_ENTRIES].sort());
  });
});

// The default twin (ADR 0957): every source and every context that places a site can be generated with the site in a package. A new source or context with no
// package variant fails here; a reason in the exemption lists (package-site-coverage.mjs) is the way to leave one out.
describe("the package site is a default twin", () => {
  it("covers every source, or says why not", () => {
    for (const source of SOURCES) {
      const exempt = PACKAGE_SITE_EXEMPT_SOURCES[source];
      const twin = packageSiteOf({ source });
      if (exempt) {
        expect(exempt.length, `${source}: a reason`).toBeGreaterThan(20);
        expect(twin.built, `${source} is exempt and has a package variant: drop the exemption`).toBeUndefined();
        continue;
      }
      expect(twin.built, `${source} has no package variant and no exemption: ${twin.error}`).toBeDefined();
    }
    for (const source of Object.keys(PACKAGE_SITE_EXEMPT_SOURCES)) expect(SOURCES, `${source}: an exemption for a source that is gone`).toContain(source);
  }, 120000);
  it("covers every context, or says why not", () => {
    for (const [context, reason] of Object.entries(PACKAGE_SITE_EXEMPT_CONTEXTS)) {
      expect(reason.length, `${context}: a reason`).toBeGreaterThan(20);
      expect(packageSiteOf({ context }).built, `${context} is exempt and has a package variant: drop the exemption`).toBeUndefined();
    }
    const contexts = [...new Set([...cellList({ recall: true }), ...legalCells({ recall: true })].map((cell) => cell.context))].sort();
    const missing = [];
    for (const context of contexts) {
      if (PACKAGE_SITE_EXEMPT_CONTEXTS[context]) continue;
      const twin = packageSiteOf({ context });
      if (!twin.built) missing.push(`${context}: ${twin.error}`);
    }
    expect(missing, "a context with no package variant and no exemption").toEqual([]);
  }, 600000);
  it("names the codes that take it all", () => {
    for (const code of Object.keys(SPEC)) if (!SPEC[code].typedAway) expect(typeof SPEC[code].call, code).toBe("function");
    expect(parentSource("props")).toBe(false || parentSource("props"));
    expect(sourcesOf("REACTIVE_WRITE_IN_OWNED_SCOPE", "signal", "direct", "body")).toContain("local");
    expect(COMPONENT_ENTRIES.every((entry) => PKG_ENTRIES.includes(entry))).toBe(true);
  });
});

describe("package-cells", () => {
  it("lists a unit per (grid, code) with the package programs' ids and the contexts they sit in", () => {
    const units = packageCells({ codes: ["CONTEXT_NOT_FOUND"] });
    expect(units).toHaveLength(1);
    expect(units[0]).toMatchObject({ grid: "mx-recall", code: "CONTEXT_NOT_FOUND" });
    expect(units[0].ids).toHaveLength(91);
    expect(units[0].ids.every((id) => /^mx-context-not-found-.*-pk-/.test(id))).toBe(true);
    expect(units[0].contexts).toEqual(["body", "memo", "render-compute", "root", "jsx", "show", "for"]);
    const both = packageCells({ codes: ["NO_OWNER_EFFECT"], entries: ["call"] });
    expect(both.map((unit) => unit.grid)).toEqual(["mx-recall", "lg-recall"]);
    for (const unit of both) expect(unit.ids.every((id) => id.endsWith("-pk-call"))).toBe(true);
    expect(packageCells({ codes: ["NO_OWNER_EFFECT"], grids: ["lg"], contexts: ["body"] })[0].contexts).toEqual(["body"]);
  });
});
