// Run: bun packages/cli/node_modules/vitest/vitest.mjs run --config packages/cli/vitest.config.mjs scripts/fuzz/api-fire-2of2.test.mjs
// Half 2 of 2 of the API-fire plan (api-fire-plan.mjs `partOf(1)`; the owner's ruling: the plan is split, the 300 s limit kept): the programs of the cells whose key hashes to
// 1 fire what their cells say on the fast runtime. api-fire-1of2.test.mjs is the other half; api-fire-plan.test.mjs holds that the two do not overlap and together are the plan.
import { describe, expect, it } from "vitest";
import { fire, partOf } from "./api-fire-plan.mjs";

describe("the API variants and contexts on the fast runtime (half 2 of 2)", () => {
  it("fire in every misuse cell, and report nothing in every legal one", async () => {
    const { count, wrong } = await fire(partOf(1));
    expect(count).toBeGreaterThan(225);
    expect(wrong).toEqual([]);
  }, 300000);
});
