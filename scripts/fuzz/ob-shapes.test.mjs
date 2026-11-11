import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { runtimeEvents } from "./classify.mjs";
import { policyOf } from "./codes.mjs";
import { buildCase, writeCase } from "./emit.mjs";
import { DEFAULT_FAST_INSTALL, runFast } from "./fast.mjs";
import { generateProgram } from "./generate.mjs";
import { hitCount } from "./hits.mjs";
import { recallLegalShapes } from "./legal.mjs";
import { BASE_ONLY_CONTEXTS, SPEC } from "./misuse.mjs";
import { censusCells, judge, recallShapes, siteOf } from "./matrix.mjs";
import { isValid } from "./model.mjs";
import { OB_CODES, OB_CONSTRUCT_NAMES_A, OB_CONSTRUCT_NAMES_C, OB_CONTEXTS, OB_CONTEXT_NAMES, OB_HOSTS, obParts, obPlacement, obShape } from "./ob-shapes.mjs";

// ADR 0963: the object-boundary shapes, a researcher's 64 Chrome false cleans on main 9e5a45080 as generator cells with their twins that never run the construct.

const shapesOf = (code, context) => (OB_CONTEXTS[context] === "misuse" ? recallShapes : recallLegalShapes)(code, context);
const textOf = (code, context) => {
  const shape = shapesOf(code, context).find((one) => one.binding === "direct" && one.layer === "none");
  const program = generateProgram(7, { profile: "misuse", misusePlan: { code, context, variant: shape.variant, binding: "direct", layer: "none", legal: OB_CONTEXTS[context] === "legal" } });
  return { program, built: buildCase(program, { id: `ob-${context}` }) };
};
const lines = (text) => text.split("\n").map((line) => line.trim());
const AWAIT = "UNTRACKED_READ_AFTER_AWAIT";
const LEAF = "CLEANUP_IN_FORBIDDEN_SCOPE";

describe("the ob contexts", () => {
  it("are 29 family A constructs on four hosts and nine family C forms on one, each with its twin, each name parsed back", () => {
    expect(OB_CONSTRUCT_NAMES_A).toHaveLength(29);
    expect(OB_CONSTRUCT_NAMES_C).toHaveLength(9);
    expect(OB_CONTEXT_NAMES).toHaveLength(29 * 4 * 2 + 9 * 2);
    for (const name of OB_CONTEXT_NAMES) {
      const parts = obParts(name);
      expect(parts, name).not.toBeNull();
      expect(OB_CONTEXTS[name]).toBe(parts.never ? "legal" : "misuse");
      expect(name).toBe(`ob-${parts.construct}-${parts.host}${parts.never ? "-never" : ""}`);
      expect(parts.family).toBe(OB_HOSTS[parts.host]);
    }
    // a construct of one family on a host of the other, an unknown host, an unknown construct, another family's name
    for (const other of ["ob-getter-apply", "ob-exp-fn-same-settled", "ob-getter-body", "ob-getters-settled", "ob-getter-settled-never-never", "pl-untrack-settled", "tw-owner-null-call-first-tracked"]) expect(obParts(other), other).toBeNull();
  });
  it("are placed for SC1002's two async computes, SC3001's three codes on the two leaves, and SC6002 and SC2008 on the apply, and for no other code", () => {
    const placement = obPlacement();
    expect(Object.keys(placement).sort()).toEqual(["CLEANUP_IN_FORBIDDEN_SCOPE", "FLUSH_IN_EFFECT_CALLBACK", "FLUSH_IN_TRACKED_QUEUE_CALLBACK", "PRIMITIVE_IN_EFFECT_CALLBACK", "PRIMITIVE_IN_FORBIDDEN_SCOPE", "UNTRACKED_READ_AFTER_AWAIT"]);
    for (const [code, hosts] of Object.entries(OB_CODES)) {
      const n = hosts.reduce((sum, host) => sum + (OB_HOSTS[host] === "A" ? 29 : 9), 0);
      expect(placement[code].misuse, code).toHaveLength(n);
      expect(placement[code].legal, code).toHaveLength(n);
      expect(placement[code].legal.every((name) => name.endsWith("-never"))).toBe(true);
      expect(placement[code].misuse.every((name) => hosts.includes(obParts(name).host))).toBe(true);
    }
    expect(OB_CODES[AWAIT]).toEqual(["memo-async", "effect-async"]);
    expect(OB_CODES[LEAF]).toEqual(["settled", "tracked"]);
    expect(OB_CODES.FLUSH_IN_EFFECT_CALLBACK).toEqual(["apply"]);
  });
  it("are cells of SPEC on their side, a few base shapes only, and not the census's", () => {
    for (const [code, { misuse, legal }] of Object.entries(obPlacement())) {
      for (const context of misuse) { expect(SPEC[code].recallContexts, `${code}/${context}`).toContain(context); expect(recallShapes(code, context).length, `${code}/${context}`).toBeGreaterThan(0); }
      for (const context of legal) { expect(SPEC[code].recallLegal, `${code}/${context}`).toContain(context); expect(recallLegalShapes(code, context).length, `${code}/${context}`).toBeGreaterThan(0); }
    }
    for (const name of OB_CONTEXT_NAMES) expect(BASE_ONLY_CONTEXTS()).toContain(name);
    // the shapes of a cell: the direct binding bare, in an `iife` or a `helper` (family C: bare or `iife`), and family A's `alias`; no mount, source, callback form or package site
    for (const [code, context, wanted] of [[AWAIT, "ob-getter-memo-async", 4], [LEAF, "ob-getter-settled", 4], ["FLUSH_IN_EFFECT_CALLBACK", "ob-exp-fn-same-apply", 2]]) {
      const shapes = recallShapes(code, context);
      expect(shapes, `${code}/${context}`).toHaveLength(wanted);
      for (const shape of shapes) { expect(obShape(context, shape)).toBe(true); expect(shape.mount || shape.source || shape.callback || shape.pkg).toBeFalsy(); }
    }
    expect(recallShapes(AWAIT, "ob-getter-memo-async").map((shape) => `${shape.binding}/${shape.layer}`).sort()).toEqual(["alias/none", "direct/helper", "direct/iife", "direct/none"]);
    expect(recallShapes("FLUSH_IN_EFFECT_CALLBACK", "ob-exp-fn-same-apply").map((shape) => `${shape.binding}/${shape.layer}`).sort()).toEqual(["direct/iife", "direct/none"]);
    expect(obShape("ob-getter-settled", { binding: "alias", layer: "iife" })).toBe(false);
    expect(obShape("ob-exp-fn-same-apply", { binding: "alias", layer: "none" })).toBe(false);
    expect(obShape("memo", { binding: "alias", layer: "iife" })).toBe(true);
    // the census crosses no other code with them
    expect(censusCells().filter(({ context }) => OB_CONTEXT_NAMES.includes(context))).toEqual([]);
  });
});

describe("the family A programs", () => {
  const run = (code, context) => {
    const { program, built } = textOf(code, context);
    expect(isValid(program), context).toBe(true);
    expect(built.sites, context).toHaveLength(1);
    return lines(built.text);
  };
  it("put the site in the body of an object's accessor, triggered by the host's callback, and keep the twin's accessor unread", () => {
    const getter = run(LEAF, "ob-getter-settled");
    expect(getter).toContain("onSettled(() => {");
    expect(getter).toContain("const o1 = { get v() {");
    expect(getter).toContain("onCleanup(() => {});");
    expect(getter).toContain("} };");
    expect(getter.indexOf("void o1.v;")).toBeGreaterThan(getter.indexOf("} };"));
    const twin = run(LEAF, "ob-getter-settled-never");
    expect(twin).toContain("const o1 = { get v() {");
    expect(twin).toContain("void o1;");
    expect(twin).not.toContain("void o1.v;");
    expect(run(LEAF, "ob-setter-tracked")).toContain("o1.v = 1;");
  });
  it("run the site after the compute's await in the async hosts", () => {
    const memo = run(AWAIT, "ob-getter-memo-async");
    expect(memo).toContain("const m1 = createMemo(async () => {");
    expect(memo.indexOf("await Promise.resolve();")).toBeLessThan(memo.indexOf("const o1 = { get v() {"));
    expect(memo).toContain("r1();");
    const effect = run(AWAIT, "ob-tostring-effect-async");
    expect(effect).toContain("createEffect(async () => {");
    expect(effect).toContain("const o1 = { toString() {");
    expect(effect).toContain("void `${o1}`;");
    expect(effect.indexOf("await Promise.resolve();")).toBeLessThan(effect.indexOf("const o1 = { toString() {"));
  });
  it("coerce, iterate, trap, construct, initialize and tag", () => {
    expect(run(LEAF, "ob-valueof-settled")).toContain("void (+o1);");
    expect(run(LEAF, "ob-toprimitive-settled")).toContain("const o1 = { [Symbol.toPrimitive]() {");
    expect(run(LEAF, "ob-tojson-settled")).toContain("void JSON.stringify(o1);");
    const iter = run(LEAF, "ob-iter-spread-tracked");
    expect(iter).toContain("const o1: Iterable<number> = { [Symbol.iterator]() {");
    expect(iter).toContain("void [...o1];");
    const next = run(LEAF, "ob-next-from-settled");
    expect(next).toContain("const o1: Iterable<number> = { [Symbol.iterator]() { return { next() {");
    expect(next).toContain("} }; } };");
    expect(next).toContain("void Array.from(o1);");
    expect(run(LEAF, "ob-iter-for-settled")).toContain("for (const _ of o1) {}");
    expect(run(LEAF, "ob-proxy-get-settled")).toContain("const px1 = new Proxy({} as Record<string, number>, { get(_t, _k) {");
    expect(run(LEAF, "ob-proxy-construct-settled")).toContain("void new px1();");
    expect(run(LEAF, "ob-proxy-apply-settled")).toContain("void px1();");
    const ctor = run(LEAF, "ob-ctor-super-settled");
    expect(ctor).toContain("class K1 {");
    expect(ctor).toContain("constructor() {");
    expect(ctor).toContain("class K2 extends K1 {");
    expect(ctor).toContain("super();");
    expect(ctor).toContain("new K2();");
    expect(run(LEAF, "ob-ctor-implicit-settled")).toContain("class K2 extends K1 {}");
    const method = run(LEAF, "ob-super-method-settled");
    expect(method).toContain("m() {");
    expect(method).toContain("super.m();");
    expect(method).toContain("new K2().m();");
    expect(run(LEAF, "ob-fn-new-settled")).toContain("new (F1 as unknown as new () => object)();");
    expect(run(LEAF, "ob-field-direct-settled")).toContain("f = onCleanup(() => {});");
    expect(run(LEAF, "ob-field-iife-settled")).toContain("f = (() => {");
    const helper = run(LEAF, "ob-field-helper-settled");
    expect(helper).toContain("const fh1 = () => {");
    expect(helper).toContain("f = fh1();");
    expect(run(LEAF, "ob-field-super-settled")).toContain("class K2 extends K1 {}");
    expect(run(LEAF, "ob-tagged-settled")).toContain("void t1`x`;");
  });
  it("keep the twin's names used and trigger nothing, whatever the construct", () => {
    for (const construct of OB_CONSTRUCT_NAMES_A) {
      const twin = run(LEAF, `ob-${construct}-settled-never`);
      expect(twin.some((line) => /^void [A-Za-z]\w*;$/.test(line)), construct).toBe(true);
      const live = run(LEAF, `ob-${construct}-settled`);
      // the construct's trigger is what the twin leaves out: a statement of the live program the twin lacks
      expect(live.filter((line) => !twin.includes(line)).length, construct).toBeGreaterThan(0);
      expect(twin.filter((line) => !live.includes(line)), construct).toEqual(twin.filter((line) => /^void [A-Za-z]\w*;$/.test(line)));
    }
  });
  it("every cell builds a neutral, valid program of one site", () => {
    for (const [code, { misuse, legal }] of Object.entries(obPlacement())) for (const context of [...misuse, ...legal]) {
      const { program, built } = textOf(code, context);
      expect(isValid(program), `${code}/${context}`).toBe(true);
      expect(built.neutral, `${code}/${context}`).toBe(true);
      expect(built.sites, `${code}/${context}`).toHaveLength(1);
    }
  });
});

describe("the family C programs", () => {
  const run = (context) => {
    const { program, built } = textOf("FLUSH_IN_EFFECT_CALLBACK", context);
    expect(isValid(program), context).toBe(true);
    return { text: lines(built.text), built };
  };
  it("export the helper and call it from the effect's apply, in the file", () => {
    const fn = run("ob-exp-fn-same-apply");
    expect(fn.text).toContain("export function hx1() {");
    expect(fn.text).toContain("hx1();");
    expect(run("ob-exp-const-same-apply").text).toContain("export const hx1 = () => {");
    const local = run("ob-local-apply");
    expect(local.text).toContain("function hx1() {");
    expect(local.text.some((line) => line.startsWith("export function hx1"))).toBe(false);
  });
  it("reach it through the module's own export, an alias, or a second module that re-exports it", () => {
    const named = run("ob-exp-fn-import-apply");
    expect(named.text).toContain("function hx1() {");
    expect(named.text).toContain("export { hx1 as hn1 };");
    expect(named.text).toContain('import { hn1 } from "./App";');
    expect(named.text).toContain("hn1();");
    const alias = run("ob-exp-const-alias-apply");
    expect(alias.text).toContain('import { hx1 as hl1 } from "./App";');
    expect(alias.text).toContain("hl1();");
    const reexport = run("ob-exp-fn-reexport-apply");
    expect(reexport.text).toContain('import { hx1 as hl1 } from "./lib";');
    expect(reexport.built.files["lib.ts"]).toBe('export { hx1 } from "./App";\n');
  });
  it("leave the helper uncalled in the twin", () => {
    for (const form of OB_CONSTRUCT_NAMES_C) {
      const twin = run(`ob-${form}-apply-never`).text;
      expect(twin.some((line) => /^void h[xnl]1;$/.test(line)), form).toBe(true);
      expect(twin.some((line) => /^h[xnl]1\(\);$/.test(line)), form).toBe(false);
    }
  });
});

// The fast runtime, one program a cell: a misuse cell reports its code at the site, and a twin reports no defect code and never runs the site (the grids and Chrome judge the rest).
describe("the ob cells on the fast runtime", () => {
  it("fire in every misuse cell, and report nothing and run nothing in every twin", async () => {
    const root = mkdtempSync(join(tmpdir(), "fuzz-ob-"));
    try {
      const plans = Object.entries(obPlacement()).flatMap(([code, { misuse, legal }]) => [...misuse, ...legal].map((context) => {
        const kind = OB_CONTEXTS[context];
        const shape = shapesOf(code, context).find((one) => one.binding === "direct" && one.layer === "none");
        return { code, context, kind, variant: shape.variant ?? null };
      }));
      const entries = plans.map((plan, index) => {
        const program = generateProgram(7000 + index, { profile: "misuse", misusePlan: { code: plan.code, context: plan.context, variant: plan.variant, binding: "direct", layer: "none", legal: plan.kind === "legal" } });
        return { plan, built: buildCase(program, { id: `ob-${index}` }) };
      });
      const records = await runFast(entries.map(({ built }) => ({ id: built.id, dir: writeCase(join(root, built.id), built), variant: "run" })), { install: DEFAULT_FAST_INSTALL, workers: 4 });
      const wrong = [];
      for (const { plan, built } of entries) {
        const runtime = records.get(built.id);
        const site = siteOf(built);
        const label = `${plan.kind} ${plan.code}/${plan.context}/${plan.variant}`;
        if (plan.kind === "misuse") {
          const verdict = judge({ code: plan.code, site, typecheck: { ok: true }, checker: { ok: true, findings: [] }, runtime });
          if (verdict.outcome !== "false-clean") wrong.push(`${label}: ${verdict.outcome} ${JSON.stringify(verdict.detail)}`);
          continue;
        }
        const reported = runtimeEvents(runtime ?? {}).map((event) => event.code).filter((code) => policyOf(code)?.disposition?.startsWith("defect"));
        if (reported.length) wrong.push(`${label}: reported ${reported.join(",")}`);
        if (hitCount(runtime?.hits ?? null, site.hit) > 0) wrong.push(`${label}: the site ran`);
      }
      expect(wrong).toEqual([]);
    } finally { rmSync(root, { recursive: true, force: true }); }
  }, 600000);
});
