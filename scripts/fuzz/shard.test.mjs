import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { SMOKE_LEGS, parse } from "./focus.mjs";
import { cellList, combined, gate, generateShape, gridShapes, recallShapes, shapeId } from "./matrix.mjs";
import { generateLegal, legalCells, legalId, legalShapes } from "./legal.mjs";
import { gridShardOptions, inShard, ledgerPart, shardEntries, shardKey, shardLine, shardOf, smokeShardOf } from "./shard.mjs";

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

  // A shard generates only its own programs by picking on the id before generation (matrix.mjs `sweep`, legal.mjs): that is the
  // same pick as generating the whole grid and keeping the shard only if the id known from the shape is the generated program's id.
  it("picks by the same id before generation as after it, on the misuse, recall and legal grids", () => {
    const options = { seed: "1" };
    const misuse = cellList({ codes: ["FLUSH_IN_ACTION"] }).flatMap((cell) => combined(gridShapes(cell.code, cell.context, ["root", "child"]), false).slice(0, 6).map((shape) => [cell, shape]));
    const recall = cellList({ codes: ["NO_OWNER_CONTEXT"], recall: true }).flatMap((cell) => recallShapes(cell.code, cell.context).slice(0, 3).map((shape) => [cell, shape]));
    expect(misuse.length).toBeGreaterThan(0);
    expect(recall.length).toBeGreaterThan(0);
    for (const [cell, shape] of [...misuse, ...recall]) expect(generateShape(options, cell, shape, 0).built.id).toBe(shapeId(cell, shape));
    const legal = legalCells({ codes: ["FLUSH_IN_ACTION"] }).flatMap((cell) => legalShapes(cell.code, ["root"], ["local"], cell.context).slice(0, 6).map((shape) => [cell, shape]));
    expect(legal.length).toBeGreaterThan(0);
    for (const [cell, shape] of legal) expect(generateLegal(options, cell, { shape }).built.id).toBe(legalId(cell, { shape }));
    const [cell] = legalCells({ codes: ["FLUSH_IN_ACTION"] });
    expect(generateLegal(options, cell, { index: 3 }).built.id).toBe(legalId(cell, { index: 3 }));
  });
});

describe("the smoke shard (ADR 1156)", () => {
  const ids = Array.from({ length: 600 }, (_, i) => `mx-case-${i}`);
  const shards = [0, 1].map((index) => ({ index, count: 2 }));
  const parts = shards.map((shard) => ids.filter((id) => inShard(shard, id)));

  it("reads --smoke-shard i/N and refuses anything else", () => {
    expect(smokeShardOf(undefined)).toBeNull();
    expect(smokeShardOf("1/2")).toEqual({ index: 1, count: 2 });
    for (const bad of ["2/2", "0/0", "-1/2", "a/2", "1", "0/2/3"]) expect(() => smokeShardOf(bad)).toThrow(/--smoke-shard takes i\/N/);
  });

  it("splits a sample's programs by the grid shard's pick: every program in exactly one shard", () => {
    expect([...parts[0], ...parts[1]].sort()).toEqual([...ids].sort());
    expect(new Set([...parts[0], ...parts[1]]).size).toBe(ids.length);
    for (const part of parts) expect(part.length).toBeGreaterThan(250);
    for (const [index, part] of parts.entries()) for (const id of part) expect(shardOf(id, 2)).toBe(index);
    // no shard asked: every id is in
    expect(ids.every((id) => inShard(null, id))).toBe(true);
  });

  it("holds each ledger program in the shard its id is in: the shards' ledgers partition the ledger, so no shard judges another's program lost", () => {
    const ledger = { grid: "mx-base", ids: new Set(ids.slice(0, 100)), entries: new Map(ids.slice(100, 300).map((id, i) => [`key-${String(i).padStart(3, "0")}`, id])), retired: new Map([["gone", "mx-case-599"]]) };
    const ledgers = shards.map((shard) => ledgerPart(ledger, shard));
    expect([...ledgers[0].ids, ...ledgers[1].ids].sort()).toEqual([...ledger.ids].sort());
    expect([...ledgers[0].entries, ...ledgers[1].entries].sort()).toEqual([...ledger.entries].sort());
    for (const [index, part] of ledgers.entries()) {
      expect(part.ids.size).toBeGreaterThan(0);
      expect(part.entries.size).toBeGreaterThan(0);
      for (const id of part.ids) expect(parts[index]).toContain(id);
      for (const [, id] of part.entries) expect(parts[index]).toContain(id);
      expect(part.retired).toBe(ledger.retired);
    }
    expect(ledgerPart(ledger, null)).toBe(ledger);
  });

  it("is asked of focus.mjs with --smoke-shard i/N: with --smoke and --sample 0, on grids the cover samples", () => {
    const leg = ["--smoke", "--smoke-leg", "rest", "--sample", "0"];
    expect(parse([...leg, "--smoke-shard", "1/2"]).smokeShard).toBe("1/2");
    expect(parse(leg).smokeShard).toBeUndefined();
    expect(() => parse([...leg, "--smoke-shard", "2/2"])).toThrow(/--smoke-shard takes i\/N/);
    expect(() => parse(["--codes", "all", "--smoke-shard", "0/2"])).toThrow(/needs --smoke/);
    expect(() => parse(["--smoke", "--smoke-leg", "rest", "--smoke-shard", "0/2"])).toThrow(/needs --sample 0/);
    // the census is sampled like the rest in the pull request's cover (all), and runs whole beside a per-code one: every shard would run all of it
    expect(parse(["--smoke", "--smoke-leg", "census", "--sample", "0", "--smoke-shard", "0/2"]).smokeShard).toBe("0/2");
    expect(() => parse(["--smoke", "--smoke-leg", "census", "--cover", "per-code", "--sample", "0", "--smoke-shard", "0/2"])).toThrow(/cannot split mx-census/);
    expect(parse(["--smoke", "--smoke-leg", "rest", "--cover", "per-code", "--sample", "0", "--smoke-shard", "0/2"]).smokeShard).toBe("0/2");
  });

  it("is run by fuzz-pr.yml as every part of each leg, so the jobs' union is the cover: a dropped or renumbered part fails here, not silently", () => {
    const workflow = readFileSync(new URL("../../.github/workflows/fuzz-pr.yml", import.meta.url), "utf8");
    // the matrix's one-line mappings: `- { leg: rest, shard: "0/2", cache: rest-0, name: smoke }`
    const jobs = [...workflow.matchAll(/^ +- \{ (leg: [^}]*) \}$/gm)].map(([, body]) => Object.fromEntries(body.split(",").map((pair) => {
      const [key, ...value] = pair.split(":");
      return [key.trim(), value.join(":").trim().replace(/^"(.*)"$/, "$1")];
    })));
    expect(jobs.length).toBeGreaterThan(1);
    // every leg runs, and nothing else does
    expect([...new Set(jobs.map((job) => job.leg))].sort()).toEqual(Object.keys(SMOKE_LEGS).sort());
    for (const leg of Object.keys(SMOKE_LEGS)) {
      const shards = jobs.filter((job) => job.leg === leg).map((job) => smokeShardOf(job.shard));
      // a leg in one job runs whole, and never beside a part of itself
      if (shards.includes(null)) { expect(shards).toHaveLength(1); continue; }
      // a leg in several jobs: one N, each index once
      const { count } = shards[0];
      expect(shards.every((shard) => shard.count === count)).toBe(true);
      expect(shards.map((shard) => shard.index).sort()).toEqual(Array.from({ length: count }, (_, index) => index));
    }
    // a name and a cache of its own: two jobs sharing one would overwrite each other's artifacts and runtime cache
    expect(new Set(jobs.map((job) => job.name)).size).toBe(jobs.length);
    expect(new Set(jobs.map((job) => job.cache)).size).toBe(jobs.length);
    // the Smoke step hands the matrix's part to focus.mjs
    expect(workflow).toContain("SHARD: ${{ matrix.shard }}");
    expect(workflow).toContain('--smoke-shard "$SHARD"');
  });
});
