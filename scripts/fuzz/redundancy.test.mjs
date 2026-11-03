// Run: bun packages/cli/node_modules/vitest/vitest.mjs run --config packages/cli/vitest.config.mjs scripts/fuzz/redundancy.test.mjs
import { describe, expect, it } from "vitest";
import { LEAN_DEMOTED, cellList, recallShapes } from "./matrix.mjs";
import { redundancy, twinOf } from "./redundancy.mjs";

describe("the demotion evidence (redundancy.mjs) and --recall-lean", () => {
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
  it("leaves the recall grids as they are unless asked, and drops only the named values when asked", () => {
    const cell = cellList({ codes: ["FLUSH_IN_EFFECT_CALLBACK"], recall: true }).find((c) => c.context === "effect-apply");
    const full = recallShapes(cell.code, cell.context);
    const lean = recallShapes(cell.code, cell.context, { lean: true });
    expect(lean.length).toBeLessThan(full.length);
    expect(lean.some((shape) => LEAN_DEMOTED.mounts.includes(shape.mount) || LEAN_DEMOTED.callbacks.includes(shape.callback))).toBe(false);
    expect(full.filter((shape) => !LEAN_DEMOTED.mounts.includes(shape.mount) && !LEAN_DEMOTED.callbacks.includes(shape.callback))).toEqual(lean);
  });
});

describe("redundancy --against <mount>", () => {
  it("pairs a recall mount's program with the same shape under the reference mount", () => {
    expect(twinOf("mx-a-body-x-direct-none-m-portal", { against: "show-signal" })).toEqual({ axis: "mount:portal", twin: "mx-a-body-x-direct-none-m-show-signal" });
    expect(twinOf("mx-a-body-x-direct-none-m-show-signal", { against: "show-signal" })).toBeNull();
    expect(twinOf("mx-a-memo-x-direct-none-cb-const", { against: "show-signal" })).toEqual({ axis: "callback:const", twin: "mx-a-memo-x-direct-none" });
  });
});
