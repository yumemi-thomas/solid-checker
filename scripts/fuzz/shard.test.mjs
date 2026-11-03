import { describe, expect, it } from "vitest";
import { parse } from "./focus.mjs";
import { gate } from "./matrix.mjs";
import { gridShardOptions, shardEntries, shardKey, shardLine, shardOf } from "./shard.mjs";

// Grid entries as matrix.mjs and legal.mjs hold them: a cell and the built case's id.
const grid = (n = 500) => Array.from({ length: n }, (_, i) => ({ cell: { code: i % 3 ? "A" : "B", context: "memo" }, built: { id: `case-${i}` } }));

describe("the grid shard", () => {
  it("partitions the grid: every program in exactly one of the N shards, in the grid's order", () => {
    const entries = grid();
    const shards = [0, 1, 2, 3].map((index) => shardEntries(entries, { index, count: 4 }));
    const ids = shards.flatMap((shard) => shard.entries.map((entry) => entry.built.id));
    expect(ids.length).toBe(entries.length);
    expect(new Set(ids).size).toBe(entries.length);
    for (const shard of shards) {
      const order = shard.entries.map((entry) => entries.indexOf(entry));
      expect(order).toEqual([...order].sort((a, b) => a - b));
      // Roughly even: the hash spreads 500 ids over 4 shards.
      expect(shard.summary.kept).toBeGreaterThan(80);
      expect(shard.summary.of).toBe(500);
    }
    expect(shards.reduce((sum, shard) => sum + shard.summary.kept, 0)).toBe(500);
  });

  it("depends on the program ids only, not on the grid's order", () => {
    const entries = grid();
    const forward = shardEntries(entries, { index: 2, count: 5 }).entries.map((entry) => entry.built.id);
    const backward = shardEntries([...entries].reverse(), { index: 2, count: 5 }).entries.map((entry) => entry.built.id).reverse();
    expect(backward).toEqual(forward);
    for (const id of forward) expect(shardOf(id, 5)).toBe(2);
    expect(shardEntries(entries, { index: 0, count: 1 }).entries).toEqual(entries);
  });

  it("reads --grid-shard i/N and refuses anything else", () => {
    expect(gridShardOptions({})).toBeNull();
    expect(gridShardOptions({ gridShard: "3/8" })).toEqual({ index: 3, count: 8 });
    for (const bad of ["8/8", "1/0", "-1/4", "a/4", "2", "1/4/2"]) expect(() => gridShardOptions({ gridShard: bad })).toThrow(/i\/N/);
    expect(() => gridShardOptions({ gridShard: "0/2", ids: "m.json" })).toThrow(/--ids/);
    expect(shardLine({ index: 1, count: 4, kept: 30, of: 120 })).toBe("shard 1/4: 30 of 120 programs; one part of the grid");
  });

  it("is gated against the same shard of the baseline only", () => {
    const matrix = { seed: "1", perCell: null, generator: 1, sampling: "grid", total: { agree: 1 }, cells: {} };
    const shard = (index, count) => ({ ...matrix, gridShard: { index, count, kept: 1, of: 4 } });
    expect(shardKey(matrix)).toBe("whole");
    expect(shardKey(shard(1, 4))).toBe("1/4");
    expect(gate(shard(1, 4), shard(1, 4)).refused).toBeUndefined();
    expect(gate(matrix, shard(1, 4)).refused).toMatch(/different parts of the grid \(baseline whole, current 1\/4\)/);
    expect(gate(shard(1, 4), matrix).refused).toMatch(/different parts/);
    expect(gate(shard(1, 4), shard(2, 4)).refused).toMatch(/different parts/);
    expect(gate(shard(1, 4), shard(1, 8)).refused).toMatch(/different parts/);
  });

  it("is asked of focus.mjs with --shard i/N, never with --smoke", () => {
    expect(parse(["--codes", "all", "--shard", "2/16"]).shard).toBe("2/16");
    for (const bad of ["16/16", "x", "1/0"]) expect(() => parse(["--codes", "all", "--shard", bad])).toThrow(/--shard takes i\/N/);
    expect(() => parse(["--smoke", "--shard", "0/2"])).toThrow(/--smoke/);
    expect(parse(["--codes", "all"]).shard).toBeUndefined();
  });
});
