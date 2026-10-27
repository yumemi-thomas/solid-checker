import { describe, expect, it } from "vitest";
import { generateProgram } from "./generate.mjs";
import { isValid, nodeCount, walk } from "./model.mjs";
import { SearchBudget, compareSize, complexity, programKey, shrink } from "./shrink.mjs";

// A fake predicate: "the program still holds a write of the given value inside a parameter default".
const holdsDefaultWrite = (value) => (program) => {
  for (const { node } of walk(program)) {
    if (node.k !== "param") continue;
    for (const inner of walk(node.default)) if (inner.node.k === "write" && inner.node.value === value) return true;
  }
  return false;
};
const find = (predicate) => { for (let seed = 1; seed < 3000; seed += 1) { const p = generateProgram(seed, { components: 3 }); if (predicate(p)) return p; } throw new Error("no seed"); };
const anyDefaultWrite = (p) => { for (const { node } of walk(p)) if (node.k === "param") for (const i of walk(node.default)) if (i.node.k === "write") return i.node.value; return null; };

describe("shrink", () => {
  const seedProgram = find((p) => anyDefaultWrite(p) !== null && nodeCount(p) > 60 && p.components.length === 3);
  const value = anyDefaultWrite(seedProgram);
  const predicate = async (program) => holdsDefaultWrite(value)(program);

  it("reduces a program while the predicate holds, keeping it valid and strictly smaller", async () => {
    const result = await shrink(seedProgram, predicate, { budget: 400 });
    expect(isValid(result.program)).toBe(true);
    expect(await predicate(result.program)).toBe(true);
    expect(nodeCount(result.program)).toBeLessThan(nodeCount(seedProgram));
    expect(result.program.components.length).toBe(1);
    expect(result.accepted).toBeGreaterThan(0);
  });

  it("never accepts a candidate that fails the predicate, and never runs an invalid or duplicate candidate", async () => {
    const seen = new Set();
    const spy = async (program) => {
      expect(isValid(program)).toBe(true);
      const key = programKey(program);
      expect(seen.has(key)).toBe(false);
      seen.add(key);
      return holdsDefaultWrite(value)(program);
    };
    const result = await shrink(seedProgram, spy, { budget: 400 });
    expect(result.attempts).toBe(seen.size);
  });

  it("is deterministic", async () => {
    const a = await shrink(seedProgram, predicate, { budget: 400 });
    const b = await shrink(seedProgram, predicate, { budget: 400 });
    expect(programKey(a.program)).toBe(programKey(b.program));
    expect(a.attempts).toBe(b.attempts);
  });

  it("stops at the execution budget", async () => {
    const result = await shrink(seedProgram, predicate, { budget: 5 });
    expect(result.attempts).toBeLessThanOrEqual(5);
    expect(result.exhausted).toBe(true);
  });

  it("returns the original when nothing smaller satisfies the predicate", async () => {
    const result = await shrink(seedProgram, async (program) => programKey(program) === programKey(seedProgram), { budget: 100 });
    expect(programKey(result.program)).toBe(programKey(seedProgram));
    expect(result.accepted).toBe(0);
  });

  it("applies the cheap filter without spending budget, and the control to every accepted program", async () => {
    let cheapCalls = 0;
    const result = await shrink(seedProgram, predicate, { budget: 400, cheap: () => { cheapCalls += 1; return false; } });
    expect(result.attempts).toBe(0);
    expect(cheapCalls).toBeGreaterThan(0);
    const controlled = await shrink(seedProgram, predicate, { budget: 400, control: async (program) => program.components.length > 1 });
    expect(controlled.program.components.length).toBeGreaterThan(1);
  });

  it("prefers less machinery: complexity orders by nodes first", () => {
    const small = generateProgram(1);
    const big = generateProgram(1, { components: 3 });
    expect(compareSize(small, big)).toBeLessThan(0);
    expect(complexity(big)[1]).toBe(3);
  });
});

describe("shrink with a batch judge", () => {
  const seedProgram = find((p) => anyDefaultWrite(p) !== null && nodeCount(p) > 60 && p.components.length === 3);
  const value = anyDefaultWrite(seedProgram);
  const predicate = async (program) => holdsDefaultWrite(value)(program);

  it("reaches the program the one-at-a-time loop reaches, whatever the batch size", async () => {
    const sequential = await shrink(seedProgram, predicate, { budget: 400 });
    for (const batchSize of [1, 2, 5, 16]) {
      const batches = [];
      const batched = await shrink(seedProgram, predicate, {
        budget: 400, batchSize,
        judgeBatch: async (programs) => { batches.push(programs.length); return Promise.all(programs.map(predicate)); },
      });
      expect(programKey(batched.program), `batch ${batchSize}`).toBe(programKey(sequential.program));
      expect(batched.accepted).toBe(sequential.accepted);
      expect(batched.attempts).toBe(sequential.attempts);
      expect(Math.max(...batches)).toBeLessThanOrEqual(batchSize);
      if (batchSize > 1) expect(batches.length).toBeLessThan(sequential.attempts);
    }
  });

  it("never judges more candidates in a batch than the budget has left, and stops at the budget", async () => {
    const sizes = [];
    const result = await shrink(seedProgram, predicate, {
      budget: 7, batchSize: 16,
      judgeBatch: async (programs) => { sizes.push(programs.length); return Promise.all(programs.map(predicate)); },
    });
    expect(result.attempts).toBeLessThanOrEqual(7);
    expect(sizes.every((size) => size <= 7)).toBe(true);
  });

  it("agrees with the sequential loop when the control rejects some accepted programs", async () => {
    const control = async (program) => program.components.length > 1 || nodeCount(program) % 3 !== 0;
    const sequential = await shrink(seedProgram, predicate, { budget: 400, control });
    const batched = await shrink(seedProgram, predicate, { budget: 400, control, batchSize: 6, judgeBatch: (programs) => Promise.all(programs.map(predicate)) });
    expect(programKey(batched.program)).toBe(programKey(sequential.program));
    expect(batched.attempts).toBe(sequential.attempts);
  });
});

describe("SearchBudget", () => {
  it("counts executions, not candidates", async () => {
    const budget = new SearchBudget(2);
    await budget.run(async () => 1);
    expect(budget.available()).toBe(true);
    await budget.run(async () => 1);
    expect(budget.available()).toBe(false);
    await expect(budget.run(async () => 1)).rejects.toThrow(/budget/);
  });
});
