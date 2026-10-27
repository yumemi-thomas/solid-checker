// Seeded, deterministic random source for the fuzzer. A seed always yields the
// same stream (mulberry32), so a program is a pure function of its seed and the
// generator version; no call here reads a clock or Math.random.
//
// `weighted` mirrors the role fast-check's `fc.oneof({ weight, arbitrary })` plays
// in solidjs/solid PR #3446 (packages/signals/tests/semantics/generate.ts); the
// implementation is ours, with no dependency on fast-check.

export function createRng(seed) {
  let state = (Number(seed) >>> 0) || 0x9e3779b9;
  const next = () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let t = state;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
  const rng = {
    /** A float in [0, 1). */
    next,
    /** An integer in [min, max], both inclusive. */
    int: (min, max) => min + Math.floor(next() * (max - min + 1)),
    chance: (probability) => next() < probability,
    pick: (items) => {
      if (!items.length) throw new Error("pick from an empty list");
      return items[Math.floor(next() * items.length)];
    },
    /** `entries` are [weight, value] pairs; a weight of 0 never wins. */
    weighted: (entries) => {
      const live = entries.filter(([weight]) => weight > 0);
      if (!live.length) throw new Error("weighted pick with no positive weight");
      let roll = next() * live.reduce((sum, [weight]) => sum + weight, 0);
      for (const [weight, value] of live) {
        roll -= weight;
        if (roll < 0) return value;
      }
      return live[live.length - 1][1];
    },
    /** A seed for an independent stream, derived without disturbing this one's shape. */
    fork: () => createRng(Math.floor(next() * 4294967296)),
  };
  return rng;
}

/** The seed of the i-th program of a run that started at `base`. */
export const programSeed = (base, index) => ((Number(base) + Math.imul(index + 1, 0x9e3779b1)) >>> 0);
