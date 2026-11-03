// Run: bun packages/cli/node_modules/vitest/vitest.mjs run --config packages/cli/vitest.config.mjs scripts/fuzz/redundancy.test.mjs
import { describe, expect, it } from "vitest";
import { legalCells, recallLegalShapes } from "./legal.mjs";
import { LEAN_DEMOTED, cellList, gate, recallLeanOf, recallShapes } from "./matrix.mjs";
import { parseArgs } from "./run.mjs";
import { redundancy, twinOf } from "./redundancy.mjs";

describe("the demotion evidence (redundancy.mjs) and the lean recall grids", () => {
  it("pairs a recall mount's or a callback form's program with its twin without the axis", () => {
    expect(twinOf("mx-a-body-x-direct-none-m-show-signal")).toEqual({ axis: "mount:show-signal", twin: "mx-a-body-x-direct-none" });
    expect(twinOf("mx-a-memo-x-direct-none-cb-const")).toEqual({ axis: "callback:const", twin: "mx-a-memo-x-direct-none" });
    // A source program is not a mount twin (a parent source always mounts as a child).
    expect(twinOf("mx-a-body-x-direct-none-m-show-later-s-prop")).toBeNull();
    expect(twinOf("mx-a-body-x-direct-none")).toBeNull();
  });
  it("counts alike only what has the same outcome and the same findings, lines aside", () => {
    const row = (id, outcome, findings = []) => ({ id, outcome, detail: { findings } });
    const table = redundancy([
      row("x", "agree", ["r/violation@5"]), row("x-m-portal", "agree", ["r/violation@9"]),
      row("y", "agree", ["r/violation@5"]), row("y-m-portal", "recall-gap", ["r/uncertifiable@9"]),
      row("z-m-portal", "agree"),
    ]);
    expect(table["mount:portal"]).toMatchObject({ programs: 3, alike: 1, differ: 1, noTwin: 1 });
  });
  it("is lean by default, and --recall-full brings every value back", () => {
    const cell = cellList({ codes: ["FLUSH_IN_EFFECT_CALLBACK"], recall: true }).find((c) => c.context === "effect-apply");
    const lean = recallShapes(cell.code, cell.context);
    const full = recallShapes(cell.code, cell.context, { lean: false });
    expect(recallShapes(cell.code, cell.context, { lean: true })).toEqual(lean);
    expect(lean.length).toBeLessThan(full.length);
    expect(lean.some((shape) => LEAN_DEMOTED.mounts.includes(shape.mount) || LEAN_DEMOTED.callbacks.includes(shape.callback))).toBe(false);
    expect(full.filter((shape) => !LEAN_DEMOTED.mounts.includes(shape.mount) && !LEAN_DEMOTED.callbacks.includes(shape.callback))).toEqual(lean);
    // Every demoted value is in the full grid (a cell that takes callbacks and mounts).
    for (const mount of LEAN_DEMOTED.mounts) expect(full.some((shape) => shape.mount === mount), mount).toBe(true);
    for (const callback of LEAN_DEMOTED.callbacks) expect(full.some((shape) => shape.callback === callback), callback).toBe(true);
  });
  it("is the same for the legal recall grid", () => {
    const cell = legalCells({ codes: ["REACTIVE_WRITE_IN_OWNED_SCOPE"], recall: true }).find((c) => c.context === "handler");
    const lean = recallLegalShapes(cell.code, cell.context);
    const full = recallLegalShapes(cell.code, cell.context, { lean: false });
    expect(lean.length).toBeLessThan(full.length);
    expect(full.filter((shape) => !LEAN_DEMOTED.mounts.includes(shape.mount) && !LEAN_DEMOTED.callbacks.includes(shape.callback))).toEqual(lean);
    for (const mount of LEAN_DEMOTED.mounts) expect(full.some((shape) => shape.mount === mount), mount).toBe(true);
  });
  it("never compares a lean recall grid with a full one (a matrix from before the flag is full)", () => {
    const matrix = (extra = {}) => ({ seed: 1, perCell: null, generator: "g", sampling: "grid", cells: {}, ...extra });
    const refused = /lean recall grid and the other a full one/;
    expect(gate(matrix(), matrix({ recallLean: true })).refused).toMatch(refused);
    expect(gate(matrix({ recallLean: true }), matrix({ recallLean: false })).refused).toMatch(refused);
    expect(gate(matrix({ recallLean: true }), matrix({ recallLean: true })).refused).toBeUndefined();
    expect(gate(matrix(), matrix({ recallLean: false })).refused).toBeUndefined();
  });
  it("reads the flags: --recall alone is lean, --recall-full is not, --recall-lean is the default said aloud, no --recall is no recall grid", () => {
    expect(recallLeanOf({ recall: true })).toBe(true);
    expect(recallLeanOf({ recall: true, recallLean: true })).toBe(true);
    expect(recallLeanOf({ recall: true, recallFull: true })).toBe(false);
    expect(recallLeanOf({})).toBe(false);
    const parsed = (...flags) => parseArgs(["--grid", "--recall", ...flags], { extraFlags: ["grid", "recall", "recall-lean", "recall-full"] });
    expect(recallLeanOf(parsed())).toBe(true);
    expect(recallLeanOf(parsed("--recall-lean"))).toBe(true);
    expect(recallLeanOf(parsed("--recall-full"))).toBe(false);
  });
});

describe("redundancy --against <mount>", () => {
  it("pairs a recall mount's program with the same shape under the reference mount", () => {
    expect(twinOf("mx-a-body-x-direct-none-m-portal", { against: "show-signal" })).toEqual({ axis: "mount:portal", twin: "mx-a-body-x-direct-none-m-show-signal" });
    expect(twinOf("mx-a-body-x-direct-none-m-show-signal", { against: "show-signal" })).toBeNull();
    expect(twinOf("mx-a-memo-x-direct-none-cb-const", { against: "show-signal" })).toEqual({ axis: "callback:const", twin: "mx-a-memo-x-direct-none" });
  });
});

describe("redundancy --against-callback <form>", () => {
  it("pairs a callback form's program with the same shape in the reference form", () => {
    expect(twinOf("mx-a-memo-x-direct-none-cb-let", { againstCallback: "const" })).toEqual({ axis: "callback:let", twin: "mx-a-memo-x-direct-none-cb-const" });
    expect(twinOf("mx-a-memo-x-direct-none-cb-const", { againstCallback: "const" })).toBeNull();
  });
});
