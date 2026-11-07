import { describe, expect, it } from "vitest";
import { cover, gridRows, ledgerIds, requiredIds, sample } from "./cover.mjs";

const row = (id, code, context, variant, layer) => ({ id, side: "misuse", code, context, variant, binding: "direct", layer });

describe("the covering sample", () => {
  const rows = [
    row("a", "X", "body", "v1", "none"), row("b", "X", "body", "v2", "iife"), row("c", "X", "root", "v1", "iife"), row("d", "X", "root", "v2", "none"),
    row("e", "Y", "body", "v1", "none"), row("f", "Y", "root", "v2", "iife"),
  ];
  const pairsOf = (list) => new Set(list.flatMap((one) => ["code", "context", "variant", "layer"].flatMap((a) => ["code", "context", "variant", "layer"].filter((b) => a < b).map((b) => `${a}=${one[a]},${b}=${one[b]}`))));
  it("covers every pair with fewer programs", () => {
    const { ids } = cover(rows);
    const chosen = rows.filter((one) => ids.includes(one.id));
    expect(pairsOf(chosen)).toEqual(pairsOf(rows));
    expect(ids.length).toBeLessThan(rows.length);
  });
  it("takes every ledger row, so a listed row is judged at every seed (Part 16: 74 rows a moved cover had dropped)", () => {
    const ledger = ledgerIds();
    expect(ledger.length).toBeGreaterThan(0);
    const required = new Set(requiredIds());
    expect(ledger.filter((id) => !required.has(id))).toEqual([]);
    expect(requiredIds({ rows: ["zz-ledger"] })).toContain("zz-ledger");
    for (const seed of [1, 2, 3]) expect(cover(rows, { seed, required: ["d"] }).ids).toContain("d");
  });
  it("is one sample for one seed and code state, and a seed moves it", () => {
    expect(cover(rows, { seed: 1 }).ids).toEqual(cover(rows, { seed: 1 }).ids);
    expect(new Set([1, 2, 3, 4, 5, 6, 7, 8].map((seed) => cover(rows, { seed }).ids.join())).size).toBeGreaterThan(1);
  });
  it("takes what is required first and covers around it", () => {
    const { ids } = cover(rows, { required: ["f"] });
    expect(ids[0]).toBe("f");
    expect(pairsOf(rows.filter((one) => ids.includes(one.id)))).toEqual(pairsOf(rows));
  });
  it("within a code asks more than across codes", () => {
    expect(cover(rows, { perCode: true }).ids.length).toBeGreaterThanOrEqual(cover(rows).ids.length);
  });
  it("reads a grid's programs without building one, under the ids the grid generates", () => {
    const shown = gridRows("mx-base", { codes: ["FLUSH_IN_ACTION"] });
    expect(shown.length).toBeGreaterThan(0);
    expect(shown.every((one) => one.id.startsWith("mx-flush-in-action-") && one.side === "misuse" && one.grid === "mx-base")).toBe(true);
    expect(gridRows("lg-base", { codes: ["STRICT_READ_UNTRACKED"] }).every((one) => one.side === "legal")).toBe(true);
  });
  it("gives each grid its share", () => {
    const { byGrid, programs } = sample({ grids: ["mx-base", "lg-base"], codes: ["STRICT_READ_UNTRACKED"] });
    expect(byGrid["mx-base"].length && byGrid["lg-base"].length).toBeGreaterThan(0);
    expect(byGrid["mx-base"].length + byGrid["lg-base"].length).toBe(programs);
  });
});
