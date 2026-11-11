import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { runtimeEvents } from "./classify.mjs";
import { policyOf } from "./codes.mjs";
import { buildCase, writeCase } from "./emit.mjs";
import { ENTRY_CODES, ENTRY_CONTEXTS, ENTRY_CONTEXT_NAMES, GE_ACTION_CODES, GE_APPLY_CODES, GE_COMPUTE_CODES, GE_LEAF_CODES, GE_LEAF_HOSTS, GE_STEPS, PD_ARGS, PD_ARG_FORMS, PD_FORMS, PD_HOSTS, PD_HOST_CODES, PD_NEVER_RUN, TW_LAYERS, entryNotVariants, entryPlacement, geLeafNames, pdNames, twParts } from "./entry-shapes.mjs";
import { DEFAULT_FAST_INSTALL, runFast } from "./fast.mjs";
import { generateProgram } from "./generate.mjs";
import { hitCount } from "./hits.mjs";
import { recallLegalShapes } from "./legal.mjs";
import { BASE_ONLY_CONTEXTS, SPEC } from "./misuse.mjs";
import { censusCells, judge, recallShapes, siteOf } from "./matrix.mjs";
import { isValid } from "./model.mjs";

// ADR 0962: the entry shapes, a researcher's five Chrome false cleans on main 29b3f9312 as generator cells with their legal twins.

const textOf = (code, context) => {
  const shapes = (ENTRY_CONTEXTS[context] === "misuse" ? recallShapes : recallLegalShapes)(code, context).filter((shape) => shape.binding === "direct" && shape.layer === "none");
  const program = generateProgram(7, { profile: "misuse", misusePlan: { code, context, variant: shapes[0].variant, binding: "direct", layer: "none", legal: ENTRY_CONTEXTS[context] === "legal" } });
  return { program, built: buildCase(program, { id: `entry-${context}` }) };
};
const lines = (text) => text.split("\n").map((line) => line.trim());
const at = (all, wanted, from = 0) => all.findIndex((line, index) => index >= from && line === wanted);

describe("the entry contexts", () => {
  it("are the action family's eight, the twice-entered family's 72, the generator-stepped family's 59 and the parameter-initializer family's 90, each name parsed back", () => {
    // nine steps in each of six hosts (an effect's apply, a memo's compute, an effect's compute, an action's step, a tracked effect, an onSettled), and the legal twin of the apply, of the memo, of the action and of the two leaf hosts
    // six forms, ten more for the argument a call passes that is `undefined` at run time (five forms, two ways, batch-10 addendum 13), a twin that passes the argument
    // and a twin that leaves the call to a handler, in each of five hosts
    expect(ENTRY_CONTEXT_NAMES).toHaveLength(8 + 2 * 2 * 3 * 2 * 3 + (9 * 6 + 5) + 5 * (6 + 5 * 2 + 2));
    expect(ENTRY_CONTEXT_NAMES.filter((name) => name.startsWith("ae-"))).toHaveLength(8);
    expect(ENTRY_CONTEXT_NAMES.filter((name) => name.startsWith("ge-"))).toHaveLength(59);
    expect(ENTRY_CONTEXT_NAMES.filter((name) => name.startsWith("pd-"))).toHaveLength(90);
    for (const name of ENTRY_CONTEXT_NAMES.filter((one) => one.startsWith("tw-"))) {
      const parts = twParts(name);
      expect(parts, name).not.toBeNull();
      expect(ENTRY_CONTEXTS[name]).toBe(parts.order === "both" ? "legal" : "misuse");
      expect(name).toBe(`tw-${parts.layer}-${parts.spelling}-${parts.order}-${parts.host}${parts.place ? `-${parts.place}` : ""}`);
    }
    for (const other of ["tw-ispending-call-first", "tw-ispending-call-first-tracked-up", "tw-untrack-call-first-tracked", "pl-ispending-tracked", "ae-helper-extra"]) expect(twParts(other)).toBeNull();
  });
  it("are placed for the codes whose tolerant layer they use, and for no other", () => {
    const placement = entryPlacement();
    expect(Object.keys(placement).sort()).toEqual(["CLEANUP_IN_FORBIDDEN_SCOPE", "FLUSH_IN_ACTION", "FLUSH_IN_EFFECT_CALLBACK", "FLUSH_IN_TRACKED_QUEUE_CALLBACK", "PRIMITIVE_IN_EFFECT_CALLBACK", "PRIMITIVE_IN_FORBIDDEN_SCOPE", "REACTIVE_WRITE_IN_OWNED_SCOPE", "UNTRACKED_READ_AFTER_AWAIT"]);
    const pd = (host, code = null) => ({ misuse: [...PD_FORMS.map((form) => `pd-${form}-${host}`), ...PD_ARG_FORMS.flatMap((form) => PD_ARGS.map((arg) => `pd-${form}-${host}-${arg}`))], legal: pdNames(host, code).filter((name) => /^pd-(passed|idle)-/.test(name)) });
    for (const code of GE_APPLY_CODES) expect(placement[code]).toEqual({ misuse: [...GE_STEPS.map((step) => `ge-${step}-apply`), ...pd("apply", code).misuse], legal: ["ge-idle-apply", ...pd("apply", code).legal] });
    for (const code of GE_COMPUTE_CODES) expect(placement[code]).toEqual({ misuse: ["memo", "effect"].flatMap((host) => GE_STEPS.map((step) => `ge-${step}-${host}`)), legal: ["ge-idle-memo"] });
    for (const code of GE_ACTION_CODES) expect(placement[code]).toEqual({ misuse: ["ae-helper", "ae-helper-named", "ae-package", "ae-delegate", ...GE_STEPS.map((step) => `ge-${step}-action`), ...pd("action", code).misuse], legal: ["ae-helper-plain", "ae-helper-named-plain", "ae-package-plain", "ae-delegate-plain", "ge-idle-action", ...pd("action", code).legal] });
    // the parameter-initializer family: the hosts name the codes it is placed for
    expect(PD_HOSTS).toEqual(["action", "apply", "tracked", "settled", "await"]);
    expect(PD_FORMS).toEqual(["value", "iife", "closure", "helper", "destructured", "ctor"]);
    expect(PD_ARG_FORMS).toEqual(["value", "iife", "closure", "helper", "ctor"]);
    expect(PD_ARGS).toEqual(["undef", "cond"]);
    expect(placement.UNTRACKED_READ_AFTER_AWAIT).toEqual(pd("await"));
    for (const [host, codes] of Object.entries(PD_HOST_CODES)) for (const code of codes) expect(pdNames(host, code).every((name) => placement[code].misuse.includes(name) || placement[code].legal.includes(name)), `${code}/${host}`).toBe(true);
    // a cleanup has no legal handler (NO_OWNER_CLEANUP): its legal twin is the one that passes the argument
    const geLeaf = (host, code) => ({ misuse: GE_STEPS.map((step) => `ge-${step}-${host}`), legal: geLeafNames(host, code) });
    expect(placement.CLEANUP_IN_FORBIDDEN_SCOPE).toEqual({
      misuse: [...geLeaf("tracked").misuse, ...geLeaf("settled").misuse, ...pd("tracked").misuse, ...pd("settled").misuse],
      legal: ["pd-passed-tracked", "pd-passed-settled"],
    });
    // the leaf codes: the generator-stepped steps in both leaf hosts (the idle twin too, but not for a cleanup)
    for (const code of ["FLUSH_IN_TRACKED_QUEUE_CALLBACK", "PRIMITIVE_IN_FORBIDDEN_SCOPE"]) {
      for (const host of GE_LEAF_HOSTS) {
        for (const name of geLeaf(host, code).misuse) expect(placement[code].misuse, `${code}/${name}`).toContain(name);
        expect(placement[code].legal, `${code}/idle ${host}`).toContain(`ge-idle-${host}`);
      }
    }
    expect(PD_NEVER_RUN).toEqual(["pd-passed-action", "pd-passed-apply", "pd-passed-tracked", "pd-passed-settled", "pd-passed-await"]);
    expect(GE_STEPS).toEqual(["forof", "spread", "next", "yield", "from", "direct", "set", "stored", "anext"]);
    expect(GE_LEAF_CODES).toEqual(["FLUSH_IN_TRACKED_QUEUE_CALLBACK", "PRIMITIVE_IN_FORBIDDEN_SCOPE", "CLEANUP_IN_FORBIDDEN_SCOPE"]);
    for (const [code, layer] of Object.entries(TW_LAYERS)) {
      const twice = (names) => names.filter((name) => twParts(name));
      expect(twice(placement[code].misuse).every((name) => twParts(name).layer === layer && twParts(name).order !== "both")).toBe(true);
      expect(twice(placement[code].legal).every((name) => twParts(name).layer === layer && twParts(name).order === "both")).toBe(true);
      expect(twice(placement[code].misuse)).toHaveLength(24);
      expect(twice(placement[code].legal)).toHaveLength(12);
    }
    expect(TW_LAYERS).toEqual({ PRIMITIVE_IN_FORBIDDEN_SCOPE: "owner-null", FLUSH_IN_TRACKED_QUEUE_CALLBACK: "ispending" });
  });
  it("are cells of SPEC on their side, base shapes only, and not the census's", () => {
    for (const [code, { misuse, legal }] of Object.entries(entryPlacement())) {
      for (const context of misuse) { expect(SPEC[code].recallContexts, `${code}/${context}`).toContain(context); expect(recallShapes(code, context).length, `${code}/${context}`).toBeGreaterThan(0); }
      for (const context of legal) { expect(SPEC[code].recallLegal, `${code}/${context}`).toContain(context); expect(recallLegalShapes(code, context).length, `${code}/${context}`).toBeGreaterThan(0); }
    }
    for (const name of ENTRY_CONTEXT_NAMES) expect(BASE_ONLY_CONTEXTS()).toContain(name);
    // the shapes of a cell are the base grid's: no mount, source, callback form or package site
    for (const shape of recallShapes("FLUSH_IN_ACTION", "ae-helper")) expect(shape.mount || shape.source || shape.callback || shape.pkg).toBeFalsy();
    // the census crosses no other code with them
    const crossed = censusCells().filter(({ context }) => ENTRY_CONTEXT_NAMES.includes(context));
    expect(crossed).toEqual([]);
  });
  it("leave the `effect` variant out of the owner-null cells: with no owner it is NO_OWNER_EFFECT, a code of its own", () => {
    const notVariants = entryNotVariants();
    expect(Object.keys(notVariants)).toEqual(["PRIMITIVE_IN_FORBIDDEN_SCOPE"]);
    for (const context of ENTRY_CODES.PRIMITIVE_IN_FORBIDDEN_SCOPE.filter((name) => twParts(name))) {
      expect(SPEC.PRIMITIVE_IN_FORBIDDEN_SCOPE.recallVariantsNotIn[context]).toContain("effect");
      const side = ENTRY_CONTEXTS[context] === "misuse" ? recallShapes : recallLegalShapes;
      expect(side("PRIMITIVE_IN_FORBIDDEN_SCOPE", context).some((shape) => shape.variant === "effect")).toBe(false);
    }
  });
});

describe("the action family's programs", () => {
  it("hand a generator literal to a project helper that makes the action, and to its twin that makes none", () => {
    const { built } = textOf("FLUSH_IN_ACTION", "ae-helper");
    const all = lines(built.text);
    expect(all).toContain("const run1 = (g: () => Generator<unknown, void, any>) => action(g);");
    expect(all).toContain("const a1 = run1(function* () {");
    expect(built.text.match(/action\(/g)).toHaveLength(1);
    expect(all).toContain("a1();");
    const twin = textOf("FLUSH_IN_ACTION", "ae-helper-plain").built;
    expect(lines(twin.text)).toContain("const run1 = (g: () => Generator<unknown, void, any>) => g;");
    expect(lines(twin.text)).toContain("void [...a1()];");
    expect(twin.text).not.toContain("action");
  });
  it("hand it by name, and delegate to the generator that holds the site", () => {
    const named = lines(textOf("FLUSH_IN_ACTION", "ae-helper-named").built.text);
    expect(named).toContain("function* g1() {");
    expect(named).toContain("const a1 = run1(g1);");
    const delegate = lines(textOf("FLUSH_IN_ACTION", "ae-delegate").built.text);
    expect(delegate).toContain("const a1 = action(function* () {");
    expect(delegate).toContain("yield* g1();");
    expect(delegate.indexOf("flush();")).toBeLessThan(delegate.indexOf("yield 1;"));
    const plain = lines(textOf("FLUSH_IN_ACTION", "ae-delegate-plain").built.text);
    expect(plain).toContain("function* o1() { yield* g1(); };");
    expect(plain).toContain("void [...o1()];");
    expect(plain.join("\n")).not.toContain("action(");
  });
  it("go through a dependency whose source says what it does with the generator", () => {
    const { built } = textOf("FLUSH_IN_ACTION", "ae-package");
    expect(built.text).toContain('import { runAction } from "fz-act";');
    expect(lines(built.text)).toContain("const a1 = runAction(function* () {");
    expect(built.packages["fz-act"]["index.js"]).toContain("return action(cb);");
    expect(built.packages["fz-act"]["index.d.ts"]).toContain("(cb: () => Generator<unknown, void, any>): () => Promise<void>");
    expect(JSON.parse(built.packages["fz-act"]["package.json"])).toMatchObject({ name: "fz-act", peerDependencies: { "solid-js": "^2.0.0-rc.21" } });
    expect(JSON.parse(built.files["oracle.json"]).packages).toEqual(["fz-act"]);
    const twin = textOf("FLUSH_IN_ACTION", "ae-package-plain").built;
    expect(twin.packages["fz-act"]["index.js"]).toContain("return cb;");
    expect(twin.packages["fz-act"]["index.js"]).not.toContain("action");
    expect(twin.packages["fz-act"]["index.d.ts"]).toContain("=> Generator<unknown, void, any>;");
  });
});

describe("the generator-stepped programs (ADR 0961, F2)", () => {
  it("step the generator that holds the site by the way the name says, in the host the name says", () => {
    const forof = lines(textOf("FLUSH_IN_EFFECT_CALLBACK", "ge-forof-apply").built.text);
    expect(forof).toContain("function* g1() {");
    expect(forof).toContain("for (const s1 of g1()) void s1;");
    expect(at(forof, "flush();")).toBeLessThan(at(forof, "yield 1;"));
    expect(forof).toContain("createEffect(() => {");
    const spread = lines(textOf("PRIMITIVE_IN_EFFECT_CALLBACK", "ge-spread-apply").built.text);
    expect(spread).toContain("void [...g1()];");
    const next = lines(textOf("FLUSH_IN_EFFECT_CALLBACK", "ge-next-apply").built.text);
    expect(next).toContain("const it1 = g1();");
    expect(next).toContain("it1.next();");
    const chain = lines(textOf("FLUSH_IN_EFFECT_CALLBACK", "ge-yield-apply").built.text);
    expect(chain).toContain("function* o1() { yield* g1(); };");
    expect(chain).toContain("for (const s1 of o1()) void s1;");
    const memo = lines(textOf("REACTIVE_WRITE_IN_OWNED_SCOPE", "ge-forof-memo").built.text);
    expect(at(memo, "const m1 = createMemo(() => {")).toBeGreaterThan(0);
    expect(memo).toContain("for (const s1 of g1()) void s1;");
    const compute = lines(textOf("REACTIVE_WRITE_IN_OWNED_SCOPE", "ge-spread-effect").built.text);
    expect(compute).toContain("void [...g1()];");
    expect(compute.join("\n")).toContain("}, (v) => {");
  });
  it("drain the generator by `Array.from`, `.next()`, `new Set` and a stored iterator's loop, and step it in an action's body (R1)", () => {
    const from = lines(textOf("FLUSH_IN_EFFECT_CALLBACK", "ge-from-apply").built.text);
    expect(from).toContain("void Array.from(g1());");
    const direct = lines(textOf("PRIMITIVE_IN_EFFECT_CALLBACK", "ge-direct-apply").built.text);
    expect(direct).toContain("g1().next();");
    const set = lines(textOf("REACTIVE_WRITE_IN_OWNED_SCOPE", "ge-set-memo").built.text);
    expect(set).toContain("void new Set(g1());");
    const stored = lines(textOf("FLUSH_IN_EFFECT_CALLBACK", "ge-stored-apply").built.text);
    expect(stored).toContain("const it1 = g1();");
    expect(stored).toContain("for (const s1 of it1) void s1;");
    // the action's body is a generator literal that steps `g`; a button starts the action
    for (const step of GE_STEPS) {
      const action = lines(textOf("FLUSH_IN_ACTION", `ge-${step}-action`).built.text);
      expect(action.some((line) => /^(async )?function\* g1\(\) \{$/.test(line)), step).toBe(true);
      expect(action, step).toContain("const a1 = action(function* () {");
      expect(action, step).toContain("a1();");
      expect(at(action, "flush();"), step).toBeLessThan(at(action, "yield 1;"));
    }
    const arrayFrom = lines(textOf("FLUSH_IN_ACTION", "ge-from-action").built.text);
    expect(arrayFrom.indexOf("const a1 = action(function* () {")).toBeLessThan(arrayFrom.indexOf("void Array.from(g1());"));
    const delegate = lines(textOf("FLUSH_IN_ACTION", "ge-yield-action").built.text);
    expect(delegate).toContain("function* o1() { yield* g1(); };");
  });
  it("discard the iterator in the host and step it in a handler in the legal twin", () => {
    const idleAction = lines(textOf("FLUSH_IN_ACTION", "ge-idle-action").built.text);
    expect(idleAction).toContain("const a1 = action(function* () {");
    expect(idleAction).toContain("g1();");
    expect(idleAction).toContain("for (const s2 of g1()) void s2;");
    const idle = lines(textOf("FLUSH_IN_EFFECT_CALLBACK", "ge-idle-apply").built.text);
    expect(idle).toContain("g1();");
    expect(idle).toContain("for (const s2 of g1()) void s2;");
    expect(idle.join("\n")).not.toContain("void [...");
    expect(at(idle, "g1();")).toBeLessThan(at(idle, "for (const s2 of g1()) void s2;"));
  });
});

describe("the parameter-initializer programs (ADR 0961, PD)", () => {
  it("put the site in the default of a function the host calls, in the form the name says", () => {
    const value = lines(textOf("FLUSH_IN_ACTION", "pd-value-action").built.text);
    expect(value.some((line) => /^const pdf1 = \(p0: unknown = flush\(\)\) => \{$/.test(line)), value.join("\n")).toBe(true);
    expect(value).toContain("const a1 = action(function* () {");
    expect(value).toContain("pdf1();");
    const iifeForm = lines(textOf("FLUSH_IN_EFFECT_CALLBACK", "pd-iife-apply").built.text);
    expect(iifeForm.some((line) => /^const pdf1 = \(p0: unknown = \(\(\) => \{$/.test(line)), iifeForm.join("\n")).toBe(true);
    expect(iifeForm).toContain("createEffect(() => {");
    const closure = lines(textOf("PRIMITIVE_IN_FORBIDDEN_SCOPE", "pd-closure-tracked").built.text);
    expect(closure.some((line) => /^const pdf1 = \(p0: \(\) => void = \(\) => \{$/.test(line)), closure.join("\n")).toBe(true);
    expect(closure).toContain("p0();");
    expect(closure).toContain("createTrackedEffect(() => {");
    const helper = lines(textOf("CLEANUP_IN_FORBIDDEN_SCOPE", "pd-helper-settled").built.text);
    expect(helper).toContain("const pdg1 = () => {");
    expect(helper).toContain("const pdf1 = (p0: unknown = pdg1()) => {");
    expect(helper).toContain("onSettled(() => {");
  });
  it("put the site in a destructuring default and a constructor's, and read after an await in the await host", () => {
    const destructured = lines(textOf("FLUSH_IN_ACTION", "pd-destructured-action").built.text);
    expect(destructured.some((line) => /^const pdf1 = \(\{ p0 = flush\(\) \}: \{ p0\?: unknown \} = \{\}\) => \{$/.test(line)), destructured.join("\n")).toBe(true);
    expect(destructured).toContain("pdf1();");
    const ctor = lines(textOf("PRIMITIVE_IN_EFFECT_CALLBACK", "pd-ctor-apply").built.text);
    expect(ctor.some((line) => /^constructor\(p0: unknown = createMemo\(/.test(line)), ctor.join("\n")).toBe(true);
    expect(ctor).toContain("new K8();");
    const awaited = lines(textOf("UNTRACKED_READ_AFTER_AWAIT", "pd-value-await").built.text);
    const memo = awaited.findIndex((line) => /^const m1 = createMemo\(async \(\) => \{$/.test(line));
    expect(memo, awaited.join("\n")).toBeGreaterThan(-1);
    expect(awaited[memo + 1]).toBe("await Promise.resolve();");
    expect(awaited[memo + 2]).toBe("pdf1();");
  });
  it("pass an argument that is undefined at run time and that no syntax proves defined, as an identifier or as the expression (batch-10 addendum 13)", () => {
    const undef = lines(textOf("UNTRACKED_READ_AFTER_AWAIT", "pd-value-await-undef").built.text);
    const memo = undef.findIndex((line) => /^const m1 = createMemo\(async \(\) => \{$/.test(line));
    expect(memo, undef.join("\n")).toBeGreaterThan(-1);
    expect(undef[memo + 1]).toBe("await Promise.resolve();");
    expect(undef[memo + 2]).toBe("const u1 = Math.random() > 2 ? 1 : undefined;");
    expect(undef[memo + 3]).toBe("pdf1(u1);");
    const cond = lines(textOf("FLUSH_IN_ACTION", "pd-helper-action-cond").built.text);
    expect(cond).toContain("pdf1(Math.random() > 2 ? 1 : undefined);");
    expect(cond).toContain("const pdf1 = (p0: unknown = pdg1()) => {");
    // the closure form's parameter is a function: its argument is a function or undefined
    const closure = lines(textOf("PRIMITIVE_IN_FORBIDDEN_SCOPE", "pd-closure-tracked-undef").built.text);
    expect(closure).toContain("const u1 = Math.random() > 2 ? () => {} : undefined;");
    expect(closure).toContain("pdf1(u1);");
    const ctor = lines(textOf("PRIMITIVE_IN_EFFECT_CALLBACK", "pd-ctor-apply-undef").built.text);
    expect(ctor).toContain("new K8(u1);");
    expect(ctor).toContain("const u1 = Math.random() > 2 ? 1 : undefined;");
    // a destructuring pattern's defaults run whatever the argument is: no such form
    expect(ENTRY_CONTEXT_NAMES.filter((name) => name.startsWith("pd-destructured-") && /-(undef|cond)$/.test(name))).toEqual([]);
    // every one is a misuse cell: the default runs
    for (const form of PD_ARG_FORMS) for (const arg of PD_ARGS) for (const host of PD_HOSTS) expect(ENTRY_CONTEXTS[`pd-${form}-${host}-${arg}`], `pd-${form}-${host}-${arg}`).toBe("misuse");
  });
  it("pass the argument in one legal twin and leave the call to a handler in the other", () => {
    const passed = lines(textOf("FLUSH_IN_ACTION", "pd-passed-action").built.text);
    expect(passed).toContain("pdf1(1);");
    const idle = lines(textOf("FLUSH_IN_EFFECT_CALLBACK", "pd-idle-apply").built.text);
    expect(idle).toContain("pdf1();");
    expect(at(idle, "createEffect(() => {")).toBeLessThan(at(idle, "pdf1();"));
    // the call is in the handler only: the effect's apply is empty
    expect(idle.filter((line) => line === "pdf1();")).toHaveLength(1);
  });
});

describe("the twice-entered programs", () => {
  const entered = (code, context) => {
    const { program, built } = textOf(code, context);
    expect(isValid(program), context).toBe(true);
    // one site, entered twice at run time
    expect(built.sites, context).toHaveLength(1);
    return lines(built.text);
  };
  it("enter the function under the layer and directly, in the order the name says", () => {
    const first = entered("FLUSH_IN_TRACKED_QUEUE_CALLBACK", "tw-ispending-call-first-tracked");
    expect(first).toContain("createTrackedEffect(() => {");
    expect(at(first, "isPending(g1);")).toBeGreaterThan(0);
    expect(at(first, "g1();")).toBeGreaterThan(at(first, "isPending(g1);"));
    const last = entered("FLUSH_IN_TRACKED_QUEUE_CALLBACK", "tw-ispending-call-last-settled");
    expect(last).toContain("onSettled(() => {");
    expect(at(last, "g1();")).toBeLessThan(at(last, "isPending(g1);"));
    const both = entered("PRIMITIVE_IN_FORBIDDEN_SCOPE", "tw-owner-null-call-both-tracked");
    expect(both.filter((line) => line === "runWithOwner(null, g1);")).toHaveLength(2);
    expect(both).not.toContain("g1();");
  });
  it("choose the entry by the helper's argument in the flag spelling: make(true) is the tolerant one", () => {
    const flag = entered("PRIMITIVE_IN_FORBIDDEN_SCOPE", "tw-owner-null-flag-first-tracked");
    expect(flag).toContain("const make1 = (p0: boolean) => {");
    expect(flag).toContain("if (p0) { runWithOwner(null, g1); } else { g1(); };");
    expect(at(flag, "make1(true);")).toBeGreaterThan(0);
    expect(at(flag, "make1(false);")).toBeGreaterThan(at(flag, "make1(true);"));
    const last = entered("PRIMITIVE_IN_FORBIDDEN_SCOPE", "tw-owner-null-flag-last-tracked");
    expect(at(last, "make1(false);")).toBeLessThan(at(last, "make1(true);"));
    const both = entered("FLUSH_IN_TRACKED_QUEUE_CALLBACK", "tw-ispending-flag-both-settled");
    expect(both.filter((line) => line === "make1(true);")).toHaveLength(2);
    expect(both).not.toContain("make1(false);");
  });
  it("put the two entries in a helper the host calls: declared in the host's callback (in) or in the component's body (out)", () => {
    const inside = entered("FLUSH_IN_TRACKED_QUEUE_CALLBACK", "tw-ispending-call-first-tracked-in");
    expect(inside).toContain("const both1 = () => {");
    expect(at(inside, "createTrackedEffect(() => {")).toBeLessThan(at(inside, "const both1 = () => {"));
    expect(at(inside, "both1();")).toBeGreaterThan(at(inside, "const both1 = () => {"));
    const outside = entered("FLUSH_IN_TRACKED_QUEUE_CALLBACK", "tw-ispending-call-first-tracked-out");
    expect(at(outside, "const both1 = () => {")).toBeLessThan(at(outside, "createTrackedEffect(() => {"));
    expect(at(outside, "const g1 = () => {")).toBeLessThan(at(outside, "const both1 = () => {"));
    const flag = entered("PRIMITIVE_IN_FORBIDDEN_SCOPE", "tw-owner-null-flag-first-settled-out");
    expect(at(flag, "make1(true);")).toBeGreaterThan(at(flag, "const both1 = () => {"));
    expect(flag).toContain("onSettled(() => {");
  });
  it("every cell builds a neutral, valid program of one site", () => {
    for (const [code, contexts] of Object.entries(ENTRY_CODES)) for (const context of contexts) {
      const { program, built } = textOf(code, context);
      expect(isValid(program), `${code}/${context}`).toBe(true);
      expect(built.neutral, `${code}/${context}`).toBe(true);
      expect(built.sites, `${code}/${context}`).toHaveLength(1);
    }
  });
});

// The fast runtime, one program a cell: a misuse cell reports its code at the site, a legal one no defect code, and its site ran (the grids and Chrome judge the rest).
describe("the entry cells on the fast runtime", () => {
  it("fire in every misuse cell, and report nothing in every legal one", async () => {
    const root = mkdtempSync(join(tmpdir(), "fuzz-entry-"));
    try {
      const plans = Object.entries(ENTRY_CODES).flatMap(([code, contexts]) => contexts.map((context) => {
        const kind = ENTRY_CONTEXTS[context];
        const shape = (kind === "misuse" ? recallShapes : recallLegalShapes)(code, context).find((one) => one.binding === "direct" && one.layer === "none");
        return { code, context, kind, variant: shape.variant ?? null };
      }));
      expect(plans).toHaveLength(Object.values(ENTRY_CODES).reduce((total, names) => total + names.length, 0));
      const entries = plans.map((plan, index) => {
        const program = generateProgram(6000 + index, { profile: "misuse", misusePlan: { code: plan.code, context: plan.context, variant: plan.variant, binding: "direct", layer: "none", legal: plan.kind === "legal" } });
        return { plan, built: buildCase(program, { id: `entry-${index}` }) };
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
        if (!PD_NEVER_RUN.includes(plan.context) && !(hitCount(runtime?.hits ?? null, site.hit) > 0)) wrong.push(`${label}: the site did not run`);
      }
      expect(wrong).toEqual([]);
    } finally { rmSync(root, { recursive: true, force: true }); }
  }, 300000);
});
