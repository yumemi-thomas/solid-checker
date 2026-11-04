import { describe, expect, it } from "vitest";
import { legalCells, recallLegalShapes, generateLegal } from "./legal.mjs";
import { LEAN_DEMOTED, cellList, generateShape, gridShapes, recallShapes, shapeSeed } from "./matrix.mjs";
import {
  CALLBACK_CONTEXTS, CALLBACK_FORMS, CALLEE_FORMS, RECALL_API_BINDINGS, RECALL_LAYERS, RECALL_TEMPLATE_CODES, RECALL_VALUE_BINDINGS, SOURCE_AXES,
  CBP_CONTEXTS, EFFECT_WINDOW_CONTEXTS, REF_CONTEXTS, SPEC, TEMPLATE_CODES, recallContextsOf, recallLegalOf, recallVariantsOf,
} from "./misuse.mjs";
import { CBP_CALLS, CBP_FORMS } from "./model.mjs";
import { RECALL_MOUNTS, mountParts } from "./mounts.mjs";
import { legalPartner, pairOutcome, siteAnswer } from "./mutation.mjs";
import { drivingPlan } from "./print.mjs";
import { DEFAULT_GRIDS, GRIDS } from "./focus.mjs";

const axisOf = (shape) => (shape.callback ? `cb:${shape.callback}` : shape.mount ? `m:${shape.mount}` : shape.source ? `s:${shape.source}`
  : RECALL_LAYERS.includes(shape.layer) ? `l:${shape.layer}` : `b:${shape.binding}`);

describe("the recall axes are opt-in", () => {
  it("adds no base cell and draws no recall-only code at random", () => {
    expect(cellList().every((cell) => TEMPLATE_CODES.includes(cell.code))).toBe(true);
    expect(TEMPLATE_CODES).not.toContain("LOADING_ON_OUTSIDE_HOLD");
    expect(RECALL_TEMPLATE_CODES).toEqual(["MISSING_EFFECT_FN", "LOADING_ON_OUTSIDE_HOLD", "LOADING_ON_ACCESSOR", "ON_FLUSH_IN_OWNED_SCOPE", "OPTIMISTIC_WRITE_BEFORE_LOAD", "CAUGHT_REACTIVE_READ"]);
    const recall = cellList({ recall: true });
    expect(recall.length).toBeGreaterThan(cellList().length);
    expect(recall.filter((cell) => recallContextsOf(cell.code).includes(cell.context)).length).toBe(recall.length - cellList().length);
  });
  it("never repeats a base shape in a base cell, and keeps the base shapes' seeds", () => {
    const cell = { code: "FLUSH_IN_EFFECT_CALLBACK", context: "effect-apply" };
    const base = new Set(gridShapes(cell.code, cell.context).map((shape) => shapeSeed(1, cell.code, cell.context, shape)));
    const recall = recallShapes(cell.code, cell.context, FULL).map((shape) => shapeSeed(1, cell.code, cell.context, shape));
    expect(recall.filter((seed) => base.has(seed))).toEqual([]);
    expect(new Set(recall).size).toBe(recall.length);
  });
  it("a recall context takes its whole base grid", () => {
    const shapes = recallShapes("FLUSH_IN_ACTION", "action-after-yield");
    expect(shapes.slice(0, gridShapes("FLUSH_IN_ACTION", "action-after-yield").length)).toEqual(gridShapes("FLUSH_IN_ACTION", "action-after-yield"));
  });
  it("focus runs the recall grids only by name", () => {
    expect(DEFAULT_GRIDS).not.toContain("mx-recall");
    expect(GRIDS["mx-recall"]).toContain("--recall");
  });
});

// `--recall-full`: every recall value (the default grid is lean, matrix.mjs LEAN_DEMOTED).
const FULL = { lean: false };

describe("every axis value crosses the base grid", () => {
  it("the lean grid reaches every value but the demoted ones, and --recall-full reaches them all", () => {
    const seenIn = (options) => new Set(cellList({ recall: true }).flatMap((cell) => recallShapes(cell.code, cell.context, options).map(axisOf)));
    const lean = seenIn({});
    const full = seenIn(FULL);
    for (const mount of RECALL_MOUNTS) expect(full).toContain(`m:${mount}`);
    for (const form of CALLBACK_FORMS) expect(full).toContain(`cb:${form}`);
    for (const mount of LEAN_DEMOTED.mounts) expect(lean, mount).not.toContain(`m:${mount}`);
    for (const form of LEAN_DEMOTED.callbacks) expect(lean, form).not.toContain(`cb:${form}`);
    const demoted = new Set([...LEAN_DEMOTED.mounts.map((mount) => `m:${mount}`), ...LEAN_DEMOTED.callbacks.map((form) => `cb:${form}`)]);
    expect([...full].filter((value) => !demoted.has(value) && !lean.has(value))).toEqual([]);
  });
  it("reaches each recall binding, callee form, layer, mount, callback form and file source", () => {
    const seen = new Set(cellList({ recall: true }).flatMap((cell) => recallShapes(cell.code, cell.context, FULL).map(axisOf)));
    for (const binding of [...RECALL_API_BINDINGS, ...RECALL_VALUE_BINDINGS, ...CALLEE_FORMS]) expect(seen).toContain(`b:${binding}`);
    for (const layer of RECALL_LAYERS) expect(seen).toContain(`l:${layer}`);
    for (const mount of RECALL_MOUNTS) expect(seen).toContain(`m:${mount}`);
    for (const form of CALLBACK_FORMS) expect(seen).toContain(`cb:${form}`);
    for (const source of SOURCE_AXES.files) expect(seen).toContain(`s:${source}`);
  });
  it("a callback form only in a context whose site is in a primitive's callback", () => {
    for (const cell of cellList({ recall: true })) {
      const callbacks = recallShapes(cell.code, cell.context, FULL).filter((shape) => shape.callback);
      if (!CALLBACK_CONTEXTS.includes(cell.context)) expect(callbacks).toEqual([]);
    }
  });
  it("one program of each (cell, axis value) builds and validates", () => {
    for (const cell of cellList({ recall: true })) {
      const seen = new Set();
      recallShapes(cell.code, cell.context, FULL).forEach((shape, index) => {
        if (seen.has(axisOf(shape))) return;
        seen.add(axisOf(shape));
        expect(() => generateShape({ seed: 1 }, cell, shape, index), `${cell.code}/${cell.context}/${axisOf(shape)}`).not.toThrow();
      });
    }
    for (const cell of legalCells()) {
      const seen = new Set();
      for (const shape of recallLegalShapes(cell.code, cell.context, FULL)) {
        if (seen.has(axisOf(shape))) continue;
        seen.add(axisOf(shape));
        expect(() => generateLegal({ seed: 1 }, cell, { shape }), `${cell.code}/${cell.context}/${axisOf(shape)}`).not.toThrow();
      }
    }
  });
});

describe("printed forms", () => {
  it("a recall API binding, a value binding and a callee form", () => {
    expect(text("FLUSH_IN_EFFECT_CALLBACK", "effect-apply", { binding: "ns-held" })["App.tsx"]).toContain("const { flush: flushN } = S2;");
    const reexport = text("FLUSH_IN_EFFECT_CALLBACK", "effect-apply", { binding: "reexport" });
    expect(reexport["solid-api.ts"]).toBe('export { flush } from "solid-js";\n');
    expect(reexport["App.tsx"]).toContain('import { flush as flushN } from "./solid-api";');
    expect(text("REACTIVE_WRITE_IN_OWNED_SCOPE", "memo", { variant: "signal", binding: "getter" })["App.tsx"]).toMatch(/const \{ useIt \} = \{ get useIt\(\) \{ return setW\d+; \} \};/);
    expect(text("FLUSH_IN_EFFECT_CALLBACK", "effect-apply", { binding: "or-assign" })["App.tsx"]).toContain("(flushSlot ||= flush)()");
  });
  it("a control layer, a module import form and a callback form", () => {
    expect(text("FLUSH_IN_EFFECT_CALLBACK", "effect-apply", { layer: "label" })["App.tsx"]).toContain("outer: {");
    const barrel = text("FLUSH_IN_EFFECT_CALLBACK", "effect-apply", { layer: "module-barrel" });
    expect(barrel["lib.ts"]).toMatch(/export \{ h\d+ \} from "\.\/helpers";/);
    expect(barrel["App.tsx"]).toMatch(/import \{ h\d+ \} from "\.\/lib";/);
    expect(text("FLUSH_IN_EFFECT_CALLBACK", "effect-apply", { layer: "module-ns" })["App.tsx"]).toMatch(/H\.h\d+\(/);
    expect(text("FLUSH_IN_EFFECT_CALLBACK", "effect-apply", { callback: "bundle" })["App.tsx"]).toMatch(/createEffect\(\(\) => \{[^]*\}, cb\d+\.cb\)/);
  });
  it("an after-await context writes the site before the await in the loop", () => {
    const app = text("UNTRACKED_READ_AFTER_AWAIT", "memo-async-loop", { variant: null })["App.tsx"];
    expect(app.indexOf("for (let i = 0; i < 2; i += 1)")).toBeGreaterThan(0);
    expect(app.indexOf("r1()")).toBeLessThan(app.indexOf("await Promise.resolve();"));
  });
  it("a later mount puts the parent's button first in the plan", () => {
    const program = generateShape({ seed: 1 }, { code: "FLUSH_IN_EFFECT_CALLBACK", context: "effect-apply" }, { variant: null, binding: "direct", layer: "none", mount: "show-later" }, 0).program;
    expect(drivingPlan(program).clicks[0]).toBe("pm1");
  });
  it("each recall mount has its parent lines; dynamic and Portal import from @solidjs/web", () => {
    for (const mount of RECALL_MOUNTS) {
      const used = new Set(); const web = new Set();
      const parts = mountParts(mount, ["<C1 />"], { index: 1, name: "C1", used, web });
      expect(parts.lines.join("\n"), mount).toMatch(/C1|D1/);
      if (["dynamic", "portal"].includes(mount)) expect(web.size, mount).toBe(1);
    }
    expect(mountParts("show", ["<C1 />"], { index: 1, name: "C1", used: new Set(), web: new Set() })).toBeNull();
  });
});

const text = (code, context, shape) => generateShape({ seed: 1 }, { code, context }, { variant: null, binding: "direct", layer: "none", ...shape }, 0).built.files;

describe("shapes rc.17 cannot observe or tsc rejects", () => {
  it("no dynamic() mount for a body's strict-read or pending read, which dynamic() leaves unlabelled", () => {
    for (const code of ["STRICT_READ_UNTRACKED", "PENDING_ASYNC_UNTRACKED_READ"]) {
      for (const context of ["body", "root"]) expect(recallShapes(code, context, FULL).filter((shape) => shape.mount === "dynamic"), `${code}/${context}`).toEqual([]);
      expect(recallShapes(code, "effect-apply", FULL).some((shape) => shape.mount === "dynamic"), code).toBe(true);
    }
  });
  it("a typed `.call` spelling for flush and useContext", () => {
    expect(text("FLUSH_IN_EFFECT_CALLBACK", "effect-apply", { binding: "call" })["App.tsx"]).toContain("flush.call<null, [], void>(null)");
    expect(text("CONTEXT_NOT_FOUND", "body", { binding: "call" })["App.tsx"]).toMatch(/useContext\.call<null, \[typeof \w+\], unknown>\(null, \w+\)/);
  });
});

describe("mutation mode", () => {
  const built = { sites: [{ code: "FLUSH_IN_EFFECT_CALLBACK", line: 10, aux: [] }] };
  const finding = (rule, kind, line = 10) => ({ rule, kind, line, endLine: line, file: "App.tsx" });
  const answer = (findings) => siteAnswer("FLUSH_IN_EFFECT_CALLBACK", built, { ok: true, findings });
  it("pairs a misuse shape with the first legal context that takes it", () => {
    expect(legalPartner("FLUSH_IN_EFFECT_CALLBACK", { variant: null, binding: "direct", layer: "none" })).toEqual({ context: "handler", variant: null });
    // A callback form needs a primitive's callback: a handler has none, a timer does.
    expect(legalPartner("FLUSH_IN_EFFECT_CALLBACK", { variant: null, binding: "direct", layer: "none", callback: "const" })).toEqual({ context: "timer", variant: null });
    expect(legalPartner("STORE_WRITE_OUTSIDE_SETTER", { variant: "assign", binding: "direct", layer: "none" }).variant).toBe("draft");
    expect(legalPartner("NO_OWNER_BOUNDARY", { variant: "loading", binding: "direct", layer: "none" })).toBeNull();
  });
  it("judges the pair at the misuse site", () => {
    const none = answer([]);
    expect(pairOutcome(answer([finding("flush-in-effect-callback", "violation")]), none).outcome).toBe("distinguished");
    expect(pairOutcome(answer([finding("flush-in-effect-callback", "uncertifiable")]), none).outcome).toBe("declined");
    expect(pairOutcome(none, none)).toEqual({ outcome: "static-miss", certified: true });
    expect(pairOutcome(answer([finding("missing-owner", "uncertifiable", 3)]), none)).toEqual({ outcome: "static-miss", certified: false });
    expect(pairOutcome(none, answer([finding("flush-in-effect-callback", "violation")])).outcome).toBe("legal-flagged");
  });
});

describe("the recall axes and one-source variants", () => {
  it("never cross a variant one source carries (the settled async memo) with the local source", () => {
    for (const cell of cellList({ recall: true })) {
      const carried = SPEC[cell.code].variantSources ?? {};
      for (const shape of recallShapes(cell.code, cell.context, FULL)) {
        if (carried[shape.variant]) expect(carried[shape.variant]).toContain(shape.source);
      }
    }
  });
});

describe("the ref contexts (recall only)", () => {
  it("are recall cells of the misuse and legal grids, never base cells", () => {
    const isRef = (cell) => cell.context.startsWith("ref");
    expect(cellList().filter(isRef)).toEqual([]);
    expect(legalCells().filter(isRef)).toEqual([]);
    expect(cellList({ recall: true }).filter(isRef).map((cell) => cell.context)).toEqual(expect.arrayContaining([...REF_CONTEXTS, "ref-factory-body"]));
    expect(legalCells({ recall: true }).filter(isRef)).toContainEqual({ code: "REACTIVE_WRITE_IN_OWNED_SCOPE", context: "ref" });
    expect(legalCells({ recall: true }).filter(isRef)).toContainEqual({ code: "NO_OWNER_EFFECT", context: "ref-factory-body" });
  });
  it("a legal recall context takes its whole base grid, and the legal grid builds it", () => {
    const shapes = recallLegalShapes("REACTIVE_WRITE_IN_OWNED_SCOPE", "ref-named");
    expect(shapes.length).toBeGreaterThan(0);
    const { built } = generateLegal({ seed: 1 }, { code: "REACTIVE_WRITE_IN_OWNED_SCOPE", context: "ref-named" }, { shape: shapes[0] });
    expect(built.files["App.tsx"]).toMatch(/<p ref=\{rf\d+\} data-view="v\d+">x<\/p>/);
  });
  it("prints each callback form on an intrinsic", () => {
    expect(text("NO_OWNER_EFFECT", "ref", { variant: "effect" })["App.tsx"]).toMatch(/<p ref=\{\(\) => \{[^]*createEffect\(/);
    expect(text("NO_OWNER_EFFECT", "ref-array", { variant: "effect" })["App.tsx"]).toMatch(/ref=\{\[\(\) => \{[^]*\}, \(\) => \{\}\]\}/);
    expect(text("NO_OWNER_EFFECT", "ref-cond", { variant: "effect" })["App.tsx"]).toMatch(/ref=\{1 \? \(\) => \{[^]*\} : undefined\}/);
    expect(text("NO_OWNER_EFFECT", "ref-factory", { variant: "effect" })["App.tsx"]).toMatch(/const dir\d+ = \(\) => \{\s+return \(\) => \{[^]*ref=\{dir\d+\(\)\}/);
    expect(text("NO_OWNER_EFFECT", "ref-row", { variant: "effect" })["App.tsx"]).toMatch(/<For each=\{\[1\]\}>[^]*<p ref=\{/);
    const body = text("REACTIVE_WRITE_IN_OWNED_SCOPE", "ref-factory-body", { variant: "signal" })["App.tsx"];
    expect(body.indexOf("setW")).toBeLessThan(body.indexOf("return () => {}"));
  });
});

describe("the owned and legal positions (recall only)", () => {
  it("are recall cells only, and keep their base cells", () => {
    const isPos = (cell) => cell.context.startsWith("pos-");
    expect(cellList().filter(isPos)).toEqual([]);
    expect(legalCells().filter(isPos)).toEqual([]);
    expect(cellList({ recall: true })).toContainEqual({ code: "STRICT_READ_UNTRACKED", context: "owner-null" });
    expect(legalCells({ recall: true })).toContainEqual({ code: "REACTIVE_WRITE_IN_OWNED_SCOPE", context: "settled" });
    expect(legalCells()).not.toContainEqual({ code: "REACTIVE_WRITE_IN_OWNED_SCOPE", context: "settled" });
    const contexts = (code) => [...SPEC[code].contexts, ...recallContextsOf(code)];
    for (const code of Object.keys(SPEC)) expect(new Set(contexts(code)).size).toBe(contexts(code).length);
  });
  it("prints each position", () => {
    expect(text("REACTIVE_WRITE_IN_OWNED_SCOPE", "pos-attr", { variant: "signal" })["App.tsx"]).toMatch(/<p data-attr=\{\(\(\) => \{[^]*setW\d+\(/);
    expect(text("REACTIVE_WRITE_IN_OWNED_SCOPE", "pos-class-method", { variant: "signal" })["App.tsx"]).toMatch(/class \w+ \{\s+m\(\) \{[^]*new \w+\(\)\.m\(\);/);
    expect(text("REACTIVE_WRITE_IN_OWNED_SCOPE", "pos-foreach", { variant: "signal" })["App.tsx"]).toContain("[1].forEach(");
    expect(text("REACTIVE_WRITE_IN_OWNED_SCOPE", "pos-handler-root", { variant: "signal" })["App.tsx"]).toMatch(/onClick=\{\(\) => \{[^]*createRoot\(/);
    expect(text("REACTIVE_WRITE_IN_OWNED_SCOPE", "pos-handler-owner", { variant: "signal" })["App.tsx"]).toMatch(/const boundOwner = createOwner\(\);[^]*onClick=\{\(\) => \{[^]*runWithOwner\(boundOwner,/);
  });
});

describe("the store operations and the setup positions (recall only)", () => {
  it("cross the store operations on the base bindings only in the recall grids", () => {
    const ops = recallVariantsOf("STORE_WRITE_OUTSIDE_SETTER");
    expect(ops).toEqual(expect.arrayContaining(["compound", "object-assign", "items-sort", "items-length", "nested-held"]));
    expect(gridShapes("STORE_WRITE_OUTSIDE_SETTER", "body").some((shape) => ops.includes(shape.variant))).toBe(false);
    expect(recallShapes("STORE_WRITE_OUTSIDE_SETTER", "body").filter((shape) => shape.variant === "items-sort").length).toBeGreaterThan(0);
    const sort = text("STORE_WRITE_OUTSIDE_SETTER", "body", { variant: "items-sort" })["App.tsx"];
    expect(sort).toContain("createStore({ v: 0, items: [1, 2] })");
    expect(sort).toMatch(/s\d+\.items\.sort\(\(a, b\) => b - a\)/);
  });
  it("prints a handler expression and flush(fn)", () => {
    expect(text("REACTIVE_WRITE_IN_OWNED_SCOPE", "pos-handler-expr", { variant: "signal" })["App.tsx"]).toMatch(/<p onClick=\{\(\(\(\) => \{[^]*setW\d+\([^]*\}\)\(\), \(\) => \{\}\)\}/);
    expect(text("REACTIVE_WRITE_IN_OWNED_SCOPE", "pos-flush-fn", { variant: "signal" })["App.tsx"]).toMatch(/flush\(\(\) => \{[^]*setW\d+\(/);
  });
});

describe("the callback props (recall only)", () => {
  const isCbp = (cell) => cell.context.startsWith("cbp-");
  it("are recall cells of the misuse and legal grids, never base cells", () => {
    expect(CBP_CONTEXTS.length).toBe(CBP_CALLS.length * CBP_FORMS.length);
    expect(cellList().filter(isCbp)).toEqual([]);
    expect(legalCells().filter(isCbp)).toEqual([]);
    expect(cellList({ recall: true })).toContainEqual({ code: "REACTIVE_WRITE_IN_OWNED_SCOPE", context: "cbp-body-spread" });
    expect(legalCells({ recall: true })).toContainEqual({ code: "REACTIVE_WRITE_IN_OWNED_SCOPE", context: "cbp-handler-forwarded" });
    expect(cellList({ recall: true })).toContainEqual({ code: "NO_OWNER_CLEANUP", context: "cbp-handler-destructured" });
    expect(legalCells({ recall: true })).toContainEqual({ code: "NO_OWNER_CLEANUP", context: "cbp-body-named" });
    // Every pass form of a call position is placed alike: rc.17 runs `run` where the child calls it, whatever the form.
    for (const code of Object.keys(SPEC)) for (const list of [recallContextsOf(code), recallLegalOf(code)]) {
      const calls = new Set(list.filter((context) => context.startsWith("cbp-")).map((context) => context.split("-")[1]));
      for (const call of calls) for (const form of CBP_FORMS) expect(list, `${code} ${call} ${form}`).toContain(form === "inline" ? `cbp-${call}` : `cbp-${call}-${form}`);
    }
  });
  it("a context is a misuse cell or a legal cell of a code, never both; a call never made is legal", () => {
    for (const code of Object.keys(SPEC)) {
      const misuse = new Set(recallContextsOf(code).filter((context) => context.startsWith("cbp-")));
      expect(recallLegalOf(code).filter((context) => misuse.has(context)), code).toEqual([]);
      expect([...misuse].filter((context) => context.startsWith("cbp-never")), code).toEqual([]);
    }
    expect(recallLegalOf("STRICT_READ_UNTRACKED")).toContain("cbp-never-spread");
  });
  it("take their base grid only", () => {
    expect(recallShapes("STRICT_READ_UNTRACKED", "cbp-body-named")).toEqual(gridShapes("STRICT_READ_UNTRACKED", "cbp-body-named"));
    expect(recallLegalShapes("REACTIVE_WRITE_IN_OWNED_SCOPE", "cbp-apply").every((shape) => !shape.mount && !shape.source && !shape.callback)).toBe(true);
  });
  it("prints each pass form", () => {
    const app = (context) => text("REACTIVE_WRITE_IN_OWNED_SCOPE", context, { variant: "signal" })["App.tsx"];
    expect(app("cbp-body")).toMatch(/<Kid(\d+) run=\{\(\) => \{\s+setW\d+\(7\);\s+\}\} \/>[^]*\nfunction Kid\1\(props: \{ run: \(\) => void \}\) \{\n  props\.run\(\);\n  return <p/);
    expect(app("cbp-body")).not.toMatch(/export function Kid/);
    expect(app("cbp-body-named")).toMatch(/function run\d+\(\) \{[^]*<Kid\d+ run=\{run\d+\} \/>/);
    expect(app("cbp-body-forwarded")).toMatch(/<Fwd(\d+) run=\{[^]*function Fwd\1\(props: \{ run: \(\) => void \}\) \{\n  return <Kid\1 run=\{props\.run\} \/>;/);
    expect(app("cbp-body-destructured")).toMatch(/function Kid\d+\(\{ run \}: \{ run: \(\) => void \}\) \{\n  run\(\);/);
    expect(app("cbp-body-spread")).toMatch(/function Fwd(\d+)[^]*return <Kid\1 \{\.\.\.props\} \/>;/);
  });
  it("prints each call position, and clicks the child's handler", () => {
    const files = (context, code = "STORE_WRITE_OUTSIDE_SETTER") => generateShape({ seed: 1 }, { code, context }, { variant: "assign", binding: "direct", layer: "none" }, 0).built;
    expect(files("cbp-jsx").files["App.tsx"]).toMatch(/return <p data-view="k\d+">\{\(props\.run\(\), "x"\)\}<\/p>;/);
    const handler = files("cbp-handler");
    expect(handler.files["App.tsx"]).toMatch(/<button data-act="(k\d+)" onClick=\{\(\) => \{ props\.run\(\); \}\}>go<\/button>/);
    expect(JSON.parse(handler.files["sites.json"]).plan.clicks).toEqual([/data-act="(k\d+)"/.exec(handler.files["App.tsx"])[1]]);
    expect(files("cbp-apply").files["App.tsx"]).toMatch(/import \{[^}]*createEffect[^}]*\} from "solid-js";[^]*createEffect\(\(\) => 1, \(\) => \{ props\.run\(\); \}\);/);
    expect(files("cbp-settled").files["App.tsx"]).toMatch(/import \{[^}]*onSettled[^}]*\} from "solid-js";[^]*onSettled\(\(\) => \{ props\.run\(\); \}\);/);
    const never = generateLegal({ seed: 1 }, { code: "STORE_WRITE_OUTSIDE_SETTER", context: "cbp-never" }, { shape: { variant: "assign", binding: "direct", layer: "none" } }).built.files["App.tsx"];
    expect(never).toMatch(/function Kid\d+\(props: \{ run: \(\) => void \}\) \{\n  return <p data-view="k\d+">x<\/p>;\n\}/);
  });
});

describe("the effect windows (recall only)", () => {
  const isWindow = (cell) => EFFECT_WINDOW_CONTEXTS.includes(cell.context);
  it("are recall cells of the misuse and legal grids, never base cells", () => {
    expect(cellList().filter(isWindow)).toEqual([]);
    expect(legalCells().filter((cell) => isWindow(cell) && cell.context !== "render-apply-guarded-rerun")).toEqual([]);
    expect(cellList({ recall: true })).toContainEqual({ code: "STRICT_READ_UNTRACKED", context: "effect-cleanup-rerun" });
    expect(cellList({ recall: true })).toContainEqual({ code: "ACTION_CALLED_IN_OWNED_SCOPE", context: "effect-cleanup-for" });
    expect(legalCells({ recall: true })).toContainEqual({ code: "REACTIVE_WRITE_IN_OWNED_SCOPE", context: "effect-cleanup-rerun" });
    expect(legalCells({ recall: true })).toContainEqual({ code: "NO_OWNER_CLEANUP", context: "effect-cleanup-for" });
  });
  it("an unowned effect out of the window is not a legal PRIMITIVE_IN_EFFECT_CALLBACK program", () => {
    const variants = new Set(recallLegalShapes("PRIMITIVE_IN_EFFECT_CALLBACK", "effect-error-arm").map((shape) => shape.variant));
    expect([...variants].sort()).toEqual(["memo", "root"]);
    expect(recallShapes("PRIMITIVE_IN_EFFECT_CALLBACK", "effect-cleanup-rerun").some((shape) => shape.variant === "effect")).toBe(true);
  });
  it("prints each window", () => {
    expect(text("STRICT_READ_UNTRACKED", "render-cleanup-rerun", { variant: "signal" })["App.tsx"]).toMatch(/createRenderEffect\(\(\) => \{\s+return t\d+\(\);[^]*\(v\) => \{\s+return \(\) => \{\s+r\d+\(\);/);
    expect(text("NO_OWNER_CLEANUP", "effect-cleanup-dispose")["App.tsx"]).toMatch(/const disp\d+ = createRoot\(\(dispose\) => \{[^]*return \(\) => \{\s+onCleanup[^]*return dispose;[^]*onClick=\{\(\) => \{\s+disp\d+\(\);/);
    expect(text("NO_OWNER_CLEANUP", "effect-cleanup-show")["App.tsx"]).toMatch(/<Show when=\{t\d+\(\)\}>\s+\{\(_u\) => \{\s+createEffect\([^]*setT\d+\(0\)/);
    expect(text("ACTION_CALLED_IN_OWNED_SCOPE", "effect-cleanup-for")["App.tsx"]).toMatch(/<For each=\{t\d+\(\) \? \[1\] : \[\]\}>/);
    expect(text("NO_OWNER_CLEANUP", "effect-error-arm")["App.tsx"]).toMatch(/createEffect\(\(\) => \{ if \(t\d+\(\) === 2\) throw new Error\("x"\); return 1; \}, \{ effect: \(\) => \{\}, error: \(\) => \{\s+onCleanup/);
    expect(text("FLUSH_IN_EFFECT_CALLBACK", "reaction")["App.tsx"]).toMatch(/const track\d+ = createReaction\(\(\) => \{\s+flush\(\);\s+\}\);\s+track\d+\(\(\) => \{\s+t\d+\(\);/);
  });
});
