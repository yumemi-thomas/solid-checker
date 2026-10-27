import { describe, expect, it } from "vitest";
import { createRng, programSeed } from "./rng.mjs";

describe("createRng", () => {
  it("is a pure function of the seed", () => {
    const a = createRng(42); const b = createRng(42);
    expect(Array.from({ length: 20 }, () => a.next())).toEqual(Array.from({ length: 20 }, () => b.next()));
    expect(createRng(1).next()).not.toBe(createRng(2).next());
  });
  it("keeps int within bounds and covers them", () => {
    const rng = createRng(7);
    const seen = new Set(Array.from({ length: 400 }, () => rng.int(2, 5)));
    expect([...seen].sort()).toEqual([2, 3, 4, 5]);
  });
  it("never picks a zero-weight entry and follows the weights", () => {
    const rng = createRng(9);
    const counts = { a: 0, b: 0, c: 0 };
    for (let i = 0; i < 3000; i += 1) counts[rng.weighted([[0, "a"], [1, "b"], [3, "c"]])] += 1;
    expect(counts.a).toBe(0);
    expect(counts.c).toBeGreaterThan(counts.b * 2);
  });
  it("refuses an empty pick and an all-zero weighting", () => {
    expect(() => createRng(1).pick([])).toThrow();
    expect(() => createRng(1).weighted([[0, "a"]])).toThrow();
  });
  it("derives distinct program seeds", () => {
    const seeds = new Set(Array.from({ length: 1000 }, (_, i) => programSeed(5, i)));
    expect(seeds.size).toBe(1000);
  });
});
