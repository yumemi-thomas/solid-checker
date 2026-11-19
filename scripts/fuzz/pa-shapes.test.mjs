import { describe, expect, it } from "vitest";
import { buildCase } from "./emit.mjs";
import { generateProgram } from "./generate.mjs";
import { recallLegalShapes } from "./legal.mjs";
import { BASE_ONLY_CONTEXTS, SPEC } from "./misuse.mjs";
import { recallShapes } from "./matrix.mjs";
import { isValid } from "./model.mjs";
import { PA_CODE, PA_CONTEXTS, PA_CONTEXT_NAMES, PA_PACKAGES, PA_PACKAGE_KINDS, paPackageFiles, paParts } from "./pa-shapes.mjs";

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
  it("are forty-seven package kinds on five hosts, misuse where the call throws, each name parsed back", () => {
    expect(PA_PACKAGE_KINDS).toHaveLength(47);
    expect(PA_CONTEXT_NAMES).toHaveLength(235);
    // Legal on every host: the package that calls nothing (`pure`), loads on a branch that never runs (`dyn`), and the symbol-keyed writer (`gwritesym`).
    const quiet = ["pure", "dyn", "gwritesym"];
    for (const name of PA_CONTEXT_NAMES) {
      const parts = paParts(name);
      expect(parts, name).not.toBeNull();
      const throws = !quiet.includes(parts.kind) && ["body", "memo", "root"].includes(parts.host);
      expect(PA_CONTEXTS[name], name).toBe(throws ? "misuse" : "legal");
    }
    expect(paParts("pa-reach")).toBeNull();
    expect(paParts("ph-literal-step")).toBeNull();
  });
  it("ship packages that name no Solid package, so the build does not read their source", () => {
    for (const [kind, names] of Object.entries(PA_PACKAGES)) {
      for (const name of names) {
        const manifest = JSON.parse(paPackageFiles(kind, name)["package.json"]);
        expect(manifest.name).toBe(name);
        expect(manifest.dependencies ?? {}).toEqual({});
        expect(manifest.peerDependencies ?? {}).toEqual({});
      }
    }
    expect(paPackageFiles("reach")["index.js"]).toContain('from "solid-js"');
    expect(paPackageFiles("pure")["index.js"]).not.toContain("import");
    expect(paPackageFiles("dyn")["index.js"]).toContain("import(name)");
    // ADR 0974: the publisher reaches Solid and writes the global; the packages that read it through the global object's own member name no Solid package.
    expect(paPackageFiles("winself", "fz-pub")["index.js"]).toContain("window.__paAct = ");
    expect(paPackageFiles("winself")["index.js"]).toContain("window.self.__paAct()");
    expect(paPackageFiles("docview")["index.js"]).toContain("document.defaultView.__paAct()");
    expect(paPackageFiles("winself")["index.js"]).not.toContain("import");
    expect(paPackageFiles("getter")["index.js"]).toContain("get now()");
    // ADR 0974: the writers replace `btoa`; the key is a literal member, a constant, a call's result (any global), or a symbol (no global).
    expect(paPackageFiles("gwrite")["index.js"]).toContain("globalThis.btoa = ");
    expect(paPackageFiles("gwritekey")["index.js"]).toContain('const KEY = "btoa";');
    expect(paPackageFiles("gwriteany")["index.js"]).toContain("Object.assign(globalThis, loadPatches())");
    expect(paPackageFiles("gwritesym")["index.js"]).toContain('Symbol.for("fz-gwritesym")');
    for (const kind of ["gwrite", "gwritekey", "gwriteany", "gwritesym"]) expect(paPackageFiles(kind)["index.js"]).toContain('from "solid-js"');
    // ADR 1108: a built-in's prototype written, and `btoa` written through an event's `view`.
    expect(paPackageFiles("gproto")["index.js"]).toContain("Float32Array.prototype.reverse = ");
    expect(paPackageFiles("gview")["index.js"]).toContain("event.view.btoa = ");
    for (const kind of ["gproto", "gview"]) expect(paPackageFiles(kind)["index.js"]).toContain('from "solid-js"');
    // ADR 1108 part 2: `exec` and `btoa` through a pattern's `view`; the routes of `reverse` read the package `gproto` ships.
    expect(paPackageFiles("gprotoimplicit")["index.js"]).toContain("RegExp.prototype.exec = ");
    expect(paPackageFiles("gprotoiter")["index.js"]).toContain("Float32Array.prototype[Symbol.iterator] = ");
    expect(paPackageFiles("gprotoany")["index.js"]).toContain("Float32Array.prototype[key] = ");
    expect(paPackageFiles("gviewpattern")["index.js"]).toContain("({ view }) => {");
    for (const kind of ["gprotocall", "gprotoapply", "gprotoreflect", "gprotodestr"]) expect(PA_PACKAGES[kind]).toEqual(["fz-gproto"]);
    // ADR 0974: the writes the scan must see besides `=`: a logical assignment, `Reflect.apply`, a writer held by a name, a bound one; and a global redefined with a getter.
    expect(paPackageFiles("gwriteand")["index.js"]).toContain("globalThis.btoa &&= ");
    expect(paPackageFiles("gwritereflect")["index.js"]).toContain("Reflect.apply(Object.assign, null, [globalThis, {");
    expect(paPackageFiles("gwritealias")["index.js"]).toContain("const assign = Object.assign;");
    expect(paPackageFiles("gwritebind")["index.js"]).toContain("Object.assign.bind(null, globalThis)");
    expect(paPackageFiles("gaccmember")["index.js"]).toContain('Object.defineProperty(window, "devicePixelRatio"');
    for (const kind of ["gwriteand", "gwritereflect", "gwritealias", "gwritebind", "gaccmember"]) expect(paPackageFiles(kind)["index.js"]).toContain('from "solid-js"');
    // The routes of the call side replace nothing: they read the package `gwrite` ships.
    for (const kind of ["gcallkey", "gcallconst", "gcalldestr", "gcallalias", "gcallarray", "gcallself", "gcallhanded"]) expect(PA_PACKAGES[kind]).toEqual(["fz-gwrite"]);
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
    // ADR 0974: the publisher is imported for its effect before the entry's package; a getter's site is the read, not a call.
    expect(text("pa-winself-body")).toContain('import "fz-pub";');
    expect(text("pa-winself-body")).toContain('import { paEnter } from "fz-winself";');
    expect(text("pa-docview-memo")).toContain('import { paEnter } from "fz-docview";');
    expect(text("pa-getter-body")).toContain('import { clock } from "fz-getter";');
    expect(text("pa-getter-body")).toMatch(/\n {2}(__h\(\d+\); )?void clock\.now;/);
    expect(Object.keys(built("pa-winself-root").files)).toEqual(expect.arrayContaining(["node_modules/fz-pub/index.js", "node_modules/fz-winself/index.js"]));
    // ADR 0974: a global a package replaces is imported for its effect and the site is the call of the global.
    expect(text("pa-gwrite-body")).toContain('import "fz-gwrite";');
    expect(text("pa-gwriteany-memo")).toMatch(/createMemo\(\(\) => \{\n\s+(__h\(\d+\); )?btoa\("x"\);/);
    expect(text("pa-gwritesym-timer")).toContain('import "fz-gwritesym";');
    expect(Object.keys(built("pa-gwritekey-root").files)).toContain("node_modules/fz-gwritekey/index.js");
    // ADR 0974: the site is the call written by another route (a declaration at the top of the body names the route where it needs one), or the read of a redefined accessor.
    expect(text("pa-gwriteand-body")).toContain('import "fz-gwriteand";');
    expect(text("pa-gcallkey-body")).toMatch(/\n {2}(__h\(\d+\); )?window\["btoa"\]\("x"\);/);
    expect(text("pa-gcallconst-memo")).toContain('const key = "btoa";');
    expect(text("pa-gcallconst-memo")).toMatch(/createMemo\(\(\) => \{\n\s+(__h\(\d+\); )?window\[key\]\("x"\);/);
    expect(text("pa-gcalldestr-root")).toContain("const { btoa: enc } = window;");
    expect(text("pa-gcallalias-timer")).toContain("const enc = window.btoa;");
    expect(text("pa-gcallarray-micro")).toContain('[btoa][0]("x");');
    expect(text("pa-gcallself-body")).toContain('window.self.btoa("x");');
    expect(text("pa-gcallhanded-memo")).toContain('((w: Window) => w.btoa("x"))(window);');
    expect(text("pa-gaccmember-body")).toMatch(/\n {2}(__h\(\d+\); )?void window\.devicePixelRatio;/);
    expect(text("pa-gaccbare-memo")).toMatch(/createMemo\(\(\) => \{\n\s+(__h\(\d+\); )?void devicePixelRatio;/);
    expect(text("pa-gacctypeof-root")).toContain("void typeof window.devicePixelRatio;");
    expect(text("pa-getterrename-body")).toContain('import { clock } from "fz-getter";');
    expect(text("pa-getterrename-body")).toContain("({ now: n } = clock);");
    expect(text("pa-getterdefault-memo")).toContain("let now = 0;");
    expect(text("pa-getterdefault-memo")).toContain("({ now = 0 } = clock);");
    expect(Object.keys(built("pa-getterdefault-root").files)).toContain("node_modules/fz-getter/index.js");
    expect(text("pa-gaccpattern-body")).toContain("let ratio = 0;");
    expect(text("pa-gaccpattern-body")).toContain("({ devicePixelRatio: ratio } = window);");
    expect(Object.keys(built("pa-gaccmember-root").files)).toContain("node_modules/fz-gaccessor/index.js");
    expect(Object.keys(built("pa-gcalldestr-body").files)).toContain("node_modules/fz-gwrite/index.js");
  });
});
