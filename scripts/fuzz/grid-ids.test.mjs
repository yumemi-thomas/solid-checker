import { describe, expect, it } from "vitest";
import { GRIDS } from "./focus-grids.mjs";
import { gridIds } from "./grid-ids.mjs";
import { SPEC } from "./misuse.mjs";

// ADR 1208 r1: the cover attributes each id to one grid (cover.mjs), so an id two grids generate drops out of one of them: ADR 1208's
// recall store cells had re-generated 536 ids of the src grids (488 mx-src, 48 lg-src), and a proved mx-src program left the cover.
describe("grid ids", () => {
  it("no id appears in two grids", () => {
    const owner = new Map();
    const shared = [];
    for (const grid of Object.keys(GRIDS)) {
      for (const code of Object.keys(SPEC)) {
        for (const id of gridIds(grid, code)) {
          const was = owner.get(id);
          if (was === undefined) owner.set(id, grid);
          else if (was !== grid && shared.length < 5) shared.push(`${id} (${was}, ${grid})`);
        }
      }
    }
    expect(shared).toEqual([]);
  }, 120_000);
});
