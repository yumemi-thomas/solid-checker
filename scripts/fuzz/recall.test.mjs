import { describe, expect, it } from "vitest";
import { legalCells, recallLegalShapes, generateLegal } from "./legal.mjs";
import { cellList, generateShape, gridShapes, recallShapes, shapeSeed } from "./matrix.mjs";
import {
  CALLBACK_CONTEXTS, CALLBACK_FORMS, CALLEE_FORMS, RECALL_API_BINDINGS, RECALL_LAYERS, RECALL_TEMPLATE_CODES, RECALL_VALUE_BINDINGS, SOURCE_AXES,
  TEMPLATE_CODES, recallContextsOf,
} from "./misuse.mjs";
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
    expect(RECALL_TEMPLATE_CODES).toEqual(["LOADING_ON_OUTSIDE_HOLD"]);
    const recall = cellList({ recall: true });
    expect(recall.length).toBeGreaterThan(cellList().length);
    expect(recall.filter((cell) => recallContextsOf(cell.code).includes(cell.context)).length).toBe(recall.length - cellList().length);
  });
  it("never repeats a base shape in a base cell, and keeps the base shapes' seeds", () => {
    const cell = { code: "FLUSH_IN_EFFECT_CALLBACK", context: "effect-apply" };
    const base = new Set(gridShapes(cell.code, cell.context).map((shape) => shapeSeed(1, cell.code, cell.context, shape)));
    const recall = recallShapes(cell.code, cell.context).map((shape) => shapeSeed(1, cell.code, cell.context, shape));
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

describe("every axis value crosses the base grid", () => {
  it("reaches each recall binding, callee form, layer, mount, callback form and file source", () => {
    const seen = new Set(cellList({ recall: true }).flatMap((cell) => recallShapes(cell.code, cell.context).map(axisOf)));
    for (const binding of [...RECALL_API_BINDINGS, ...RECALL_VALUE_BINDINGS, ...CALLEE_FORMS]) expect(seen).toContain(`b:${binding}`);
    for (const layer of RECALL_LAYERS) expect(seen).toContain(`l:${layer}`);
    for (const mount of RECALL_MOUNTS) expect(seen).toContain(`m:${mount}`);
    for (const form of CALLBACK_FORMS) expect(seen).toContain(`cb:${form}`);
    for (const source of SOURCE_AXES.files) expect(seen).toContain(`s:${source}`);
  });
  it("a callback form only in a context whose site is in a primitive's callback", () => {
    for (const cell of cellList({ recall: true })) {
      const callbacks = recallShapes(cell.code, cell.context).filter((shape) => shape.callback);
      if (!CALLBACK_CONTEXTS.includes(cell.context)) expect(callbacks).toEqual([]);
    }
  });
  it("one program of each (cell, axis value) builds and validates", () => {
    for (const cell of cellList({ recall: true })) {
      const seen = new Set();
      recallShapes(cell.code, cell.context).forEach((shape, index) => {
        if (seen.has(axisOf(shape))) return;
        seen.add(axisOf(shape));
        expect(() => generateShape({ seed: 1 }, cell, shape, index), `${cell.code}/${cell.context}/${axisOf(shape)}`).not.toThrow();
      });
    }
    for (const cell of legalCells()) {
      const seen = new Set();
      for (const shape of recallLegalShapes(cell.code, cell.context)) {
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
      for (const context of ["body", "root"]) expect(recallShapes(code, context).filter((shape) => shape.mount === "dynamic"), `${code}/${context}`).toEqual([]);
      expect(recallShapes(code, "effect-apply").some((shape) => shape.mount === "dynamic"), code).toBe(true);
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
