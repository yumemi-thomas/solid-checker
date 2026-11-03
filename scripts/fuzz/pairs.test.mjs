// Run: bun packages/cli/node_modules/vitest/vitest.mjs run --config packages/cli/vitest.config.mjs scripts/fuzz/pairs.test.mjs
// The pair grids (`--pairs`, matrix.mjs pairShapes): enumerated pairs of two axes, on the base cells, ids of their own.
import { describe, expect, it } from "vitest";
import { DEFAULT_GRIDS, GRIDS } from "./focus.mjs";
import { generateLegal, pairLegalCells } from "./legal.mjs";
import { cellList, gate, generateShape, pairShapes, pairsOption } from "./matrix.mjs";
import { PAIR_LAYERS, PAIR_SETS, SPEC } from "./misuse.mjs";
import { TWIN_LAYERS } from "./twins.mjs";

describe("the pair grids", () => {
  it("are asked for by name, never by default", () => {
    expect(GRIDS["mx-pairs"]).toEqual(["matrix.mjs", "--pairs", "all"]);
    expect(DEFAULT_GRIDS).not.toContain("mx-pairs");
    expect(DEFAULT_GRIDS).not.toContain("lg-pairs");
    expect(pairsOption("all")).toEqual(PAIR_SETS);
    expect(() => pairsOption("mount-source,nope")).toThrow(/unknown pair set nope/);
  });
  it("cross a parent's later or row mount with the values it creates, in one program", () => {
    const shapes = pairShapes("STRICT_READ_UNTRACKED", "body", { sets: ["mount-source"] });
    expect(shapes.some((shape) => shape.mount === "show-later" && shape.source === "prop")).toBe(true);
    // `dynamic()` passes no props: only a module hook takes it.
    expect(shapes.filter((shape) => shape.mount === "dynamic").every((shape) => shape.source === "module-hook")).toBe(true);
    const { built } = generateShape({ seed: 1 }, { code: "STRICT_READ_UNTRACKED", context: "body" }, { variant: "signal", binding: "direct", layer: "none", mount: "show-later", source: "prop" }, 0);
    expect(built.text).toContain("<Show when={on1()}>");
    expect(built.text).toContain("<C1 r1={r1} setR1={setR1} />");
    expect(built.plan.clicks[0]).toBe("pm1");
  });
  it("move a later mount of a settled async source to the legal grid, in the code's misuse contexts", () => {
    const later = (shape) => shape.mount === "show-later" && shape.source.startsWith("async-");
    expect(pairShapes("PENDING_ASYNC_UNTRACKED_READ", "body", { sets: ["mount-source"] }).some(later)).toBe(false);
    const legal = pairShapes("PENDING_ASYNC_UNTRACKED_READ", "body", { legal: true });
    expect(legal.length).toBeGreaterThan(0);
    expect(legal.every(later)).toBe(true);
    expect(pairLegalCells({ codes: ["PENDING_ASYNC_UNTRACKED_READ"] }).some((cell) => cell.context === "body")).toBe(true);
    expect(SPEC.PENDING_ASYNC_UNTRACKED_READ.legal).not.toContain("body");
    expect(generateLegal({ seed: 1 }, { code: "PENDING_ASYNC_UNTRACKED_READ", context: "body" }, { shape: legal[0] }).built.id).toMatch(/-m-show-later-s-async-/);
  });
  it("write a hand-off inside the module helper as one composite layer the twins classify, and pair callbacks with hand-offs", () => {
    expect(PAIR_LAYERS).toContain("handoff-foreach+module-barrel");
    for (const layer of PAIR_LAYERS) expect(TWIN_LAYERS[layer]).toBeTruthy();
    const { built } = generateShape({ seed: 1 }, { code: "NO_OWNER_CLEANUP", context: "handler" }, { variant: null, binding: "direct", layer: "handoff-foreach+module-barrel" }, 0);
    expect(built.text).toContain('from "./lib"');
    expect(built.text).toContain("forEach(");
    const callback = pairShapes("FLUSH_IN_EFFECT_CALLBACK", "effect-apply", { sets: ["callback-handoff"] });
    expect(new Set(callback.map((shape) => shape.callback))).toEqual(new Set(["bundle", "alias", "shared"]));
  });
  it("take no recall-only context, and a pair grid is gated only against a pair grid of the same sets", () => {
    expect(pairShapes("NO_OWNER_CLEANUP", "async-loop")).toEqual([]);
    const matrix = (pairs) => ({ seed: 1, perCell: null, generator: 2, sampling: "grid", ...(pairs ? { pairs } : {}), total: {}, cells: {} });
    expect(gate(matrix(null), matrix(PAIR_SETS)).refused).toMatch(/differ in pairs/);
    expect(gate(matrix(PAIR_SETS), matrix(PAIR_SETS)).ok).toBe(true);
    expect(cellList().every((cell) => SPEC[cell.code].contexts.includes(cell.context))).toBe(true);
  });
});
