import { describe, expect, it } from "vitest";
import { parse } from "./focus.mjs";
import { gate } from "./matrix.mjs";
import { BIG_CODES, gridSampleOptions, sampleEntries, sampleLine, stratumOf } from "./sample.mjs";

// Grid entries as matrix.mjs and legal.mjs hold them: a cell, the program's misuse plan, the built case's id.
const entry = (code, context, layer, variant, n) => ({ cell: { code, context }, program: { components: [{ misuse: { layer, variant, binding: `b${n}` } }] }, built: { id: `${code}-${context}-${layer}-${variant}-${n}` } });
const grid = () => {
  const out = [];
  for (const context of ["memo", "body"]) for (const layer of ["none", "iife"]) for (let n = 0; n < 40; n += 1) out.push(entry("BIG", context, layer, "signal", n));
  for (let n = 0; n < 3; n += 1) out.push(entry("BIG", "memo", "none", "store", n));   // a small stratum
  for (let n = 0; n < 10; n += 1) out.push(entry("SMALL", "memo", "none", "signal", n));
  return out;
};

describe("the grid sample", () => {
  it("keeps the fraction of every (code, context, layer, variant) stratum of the sampled codes, at least k each, the rest whole", () => {
    const { entries, summary } = sampleEntries(grid(), { fraction: 0.1, seed: "1", min: 2, codes: ["BIG"] });
    const count = (pred) => entries.filter(pred).length;
    expect(count((e) => e.cell.code === "SMALL")).toBe(10);
    for (const context of ["memo", "body"]) for (const layer of ["none", "iife"]) expect(count((e) => stratumOf(e) === `BIG/${context}/${layer}/signal`)).toBe(4);
    expect(count((e) => stratumOf(e) === "BIG/memo/none/store")).toBe(2);   // ceil(0.3) = 1, raised to k = 2
    expect(summary).toEqual({ fraction: 0.1, seed: "1", min: 2, codes: ["BIG"], strata: 5, kept: 18, of: 163 });
    expect(sampleLine(summary)).toMatch(/^sampled 18 of 163 programs of BIG .*not a full run$/);
  });

  it("is seeded and reproducible, independent of the grid's order, and keeps a grid's order", () => {
    const picked = (entries, seed) => sampleEntries(entries, { fraction: 0.25, seed, min: 1, codes: null }).entries.map((e) => e.built.id);
    const a = picked(grid(), "7");
    expect(picked(grid(), "7")).toEqual(a);
    expect(picked([...grid()].reverse(), "7").sort()).toEqual([...a].sort());
    expect(picked(grid(), "8")).not.toEqual(a);
    const order = grid().map((e) => e.built.id);
    expect(a).toEqual(order.filter((id) => a.includes(id)));
  });

  it("is a full grid at fraction 1, and reads its options strictly", () => {
    expect(sampleEntries(grid(), { fraction: 1, seed: "1", min: 1, codes: null }).entries).toHaveLength(173);
    expect(gridSampleOptions({})).toBeNull();
    expect(gridSampleOptions({ gridSample: "0.1", gridSampleCodes: "A,B" })).toEqual({ fraction: 0.1, seed: "1", min: 1, codes: ["A", "B"] });
    expect(() => gridSampleOptions({ gridSample: "0" })).toThrow(/fraction/);
    expect(() => gridSampleOptions({ gridSample: "0.1", gridSampleMin: "-1" })).toThrow(/count/);
    expect(() => gridSampleOptions({ gridSample: "0.1", ids: "m.json" })).toThrow(/--ids/);
    expect(BIG_CODES).toEqual(["STORE_WRITE_OUTSIDE_SETTER", "REACTIVE_WRITE_IN_OWNED_SCOPE"]);
  });

  it("is refused by the matrix gate, on either side", () => {
    const matrix = { seed: "1", perCell: null, generator: 1, sampling: "grid", total: { agree: 1 }, cells: {} };
    expect(gate(matrix, { ...matrix, gridSample: { fraction: 0.1 } }).refused).toMatch(/current matrix is a sample/);
    expect(gate({ ...matrix, gridSample: { fraction: 0.1 } }, matrix).refused).toMatch(/baseline matrix is a sample/);
    expect(gate(matrix, matrix).refused).toBeUndefined();
  });

  it("is asked of focus.mjs with --sample-big, never with --smoke", () => {
    expect(parse(["--codes", "all", "--sample-big", "0.1", "--sample-big-min", "2"])).toMatchObject({ sampleBig: "0.1", sampleBigMin: "2" });
    expect(() => parse(["--smoke", "--sample-big", "0.1"])).toThrow(/--smoke/);
    expect(() => parse(["--codes", "A", "--sample-big", "1.5"])).toThrow(/fraction/);
    expect(() => parse(["--codes", "A", "--sample-codes", "B"])).toThrow(/need --sample-big/);
    expect(parse(["--codes", "all"]).sampleBig).toBeUndefined();
  });
});
