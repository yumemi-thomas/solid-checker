// Run: bun packages/cli/node_modules/vitest/vitest.mjs run --config packages/cli/vitest.config.mjs scripts/fuzz/api-fire-2of3.test.mjs
// Part 2 of 3 of the API-fire plan (api-fire-plan.mjs `partOf(1)`; the owner's ruling: the plan is split, the 300 s limit kept; three parts since ADR 0990, when the named
// primitive layers took the plan past 2,300 programs): the programs of the cells whose key hashes to 1 fire what their cells say on the fast runtime. The other parts are
// the other `api-fire-*of3.test.mjs` files; api-fire-plan.test.mjs holds that the parts do not overlap and together are the plan.
import { describe, expect, it } from "vitest";
import { fire, partOf } from "./api-fire-plan.mjs";

describe("the API variants and contexts on the fast runtime (part 2 of 3)", () => {
  it("fire in every misuse cell, and report nothing in every legal one", async () => {
    const { count, wrong } = await fire(partOf(1));
    expect(count).toBeGreaterThan(225);
    expect(wrong).toEqual([]);
  }, 300000);
});
