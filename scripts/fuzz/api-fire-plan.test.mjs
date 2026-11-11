// The split of the API-fire plan (api-fire-plan.mjs): the two halves api-fire-1of2.test.mjs and api-fire-2of2.test.mjs run have no program in common and together are the plan the one
// test had (`plans()` is that plan, moved unchanged, with each entry's place in it), and the halves are balanced enough that each stays under the 300 s limit it keeps. The cells'
// shapes the one test also held are here (no runtime).
import { describe, expect, it } from "vitest";
import { halfOf, keyOf, partOf, plans, PARTS } from "./api-fire-plan.mjs";
import { SPEC } from "./misuse.mjs";
import { recallShapes } from "./matrix.mjs";
import { recallLegalShapes } from "./legal.mjs";

describe("the API-fire plan and its two halves", () => {
  const whole = plans();
  const halves = Array.from({ length: PARTS }, (_, part) => partOf(part, whole));

  it("is the one test's plan: more than 450 programs, one entry a key, numbered by place", () => {
    expect(whole.length).toBeGreaterThan(450);
    expect(new Set(whole.map(keyOf)).size).toBe(whole.length);
    expect(whole.map((entry) => entry.index)).toEqual(whole.map((_, index) => index));
    expect(new Set(whole.map((entry) => entry.kind))).toEqual(new Set(["misuse", "legal"]));
  });

  it("splits into two halves that do not overlap", () => {
    expect(halves.length).toBe(2);
    const [first, second] = halves.map((half) => new Set(half.map((entry) => entry.index)));
    expect([...first].filter((index) => second.has(index))).toEqual([]);
    const keys = halves.map((half) => new Set(half.map(keyOf)));
    expect([...keys[0]].filter((key) => keys[1].has(key))).toEqual([]);
    // an entry is in the half its key hashes to, and in no other
    for (const [part, half] of halves.entries()) for (const entry of half) expect(halfOf(entry)).toBe(part);
  });

  it("whose union is the plan, each program once, as it was (same place, same cell, same variant)", () => {
    const union = halves.flat().sort((a, b) => a.index - b.index);
    expect(union.length).toBe(whole.length);
    expect(union).toEqual(whole);
  });

  it("holds both kinds of cell in each half, and neither is much the larger (each keeps the one limit)", () => {
    for (const half of halves) {
      expect(new Set(half.map((entry) => entry.kind))).toEqual(new Set(["misuse", "legal"]));
      expect(half.length).toBeGreaterThan(whole.length * 0.4);
      expect(half.length).toBeLessThan(whole.length * 0.6);
    }
  });
});

describe("the API cells' shapes", () => {
  it("a lazy-memo or deferred-apply cell takes its base shapes only", () => {
    const shapes = recallShapes("STRICT_READ_UNTRACKED", "effect-apply-defer-rerun");
    expect(shapes.length).toBeGreaterThan(0);
    expect(shapes.every((shape) => !shape.mount && !shape.callback && !shape.source && ["none", "closure", "helper", "iife", "closure+helper", "module-helper"].includes(shape.layer))).toBe(true);
  });
  it("a legal cell only some variants take (an owned-write signal in a body) takes only those", () => {
    expect(SPEC.REACTIVE_WRITE_IN_OWNED_SCOPE.variantOnlyLegal.body.sort()).toEqual(["optimistic", "owned-write"]);
    const shapes = recallLegalShapes("REACTIVE_WRITE_IN_OWNED_SCOPE", "body");
    expect(new Set(shapes.map((shape) => shape.variant))).toEqual(new Set(["optimistic", "owned-write"]));
    expect(SPEC.REACTIVE_WRITE_IN_OWNED_SCOPE.legal).not.toContain("body");
    expect(SPEC.STRICT_READ_UNTRACKED.variantOnlyLegal.settled.sort()).toEqual(["is-pending", "latest"]);
  });
});
