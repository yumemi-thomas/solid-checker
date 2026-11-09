import { describe, expect, it } from "vitest";
import { censusIds, cover, familyIds, gridRows, ledgerIds, requiredIds, sample, wholeGrid } from "./cover.mjs";

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
  it("asks a recall program across codes only for its pairs with its code, and within a code for every pair (ADR 0916)", () => {
    const four = (grid) => [["a", "l1"], ["b", "l2"], ["a", "l2"], ["b", "l1"]].map(([context, layer], at) => ({ ...row(`${grid}-${at}`, "X", context, "v1", layer), grid }));
    expect(cover(four("mx-base")).ids.length).toBe(4);
    expect(cover(four("mx-recall")).ids.length).toBe(2);
    expect(cover(four("lg-recall")).ids.length).toBe(2);
    expect(cover(four("mx-recall"), { perCode: true }).ids.length).toBe(4);
  });
  it("takes the census into the pull request's sample, every (code, context) pair of it, and runs it whole beside the per-code one", () => {
    expect(wholeGrid("mx-census", "all")).toBe(false);
    expect(wholeGrid("mx-census", "per-code")).toBe(true);
    expect(wholeGrid("mx-census", undefined)).toBe(true);
    expect(wholeGrid("mx-recall", "per-code")).toBe(false);
    const codes = ["STRICT_READ_UNTRACKED"];
    const census = gridRows("mx-census", { codes });
    expect(census.map((one) => one.id)).toEqual(censusIds({ codes }));
    const { byGrid } = sample({ grids: ["mx-census"], codes });
    const chosen = new Set(byGrid["mx-census"]);
    const pairsOf = (list) => new Set(list.map((one) => `${one.code}|${one.context}`));
    expect(pairsOf(census.filter((one) => chosen.has(one.id)))).toEqual(pairsOf(census));
  });
  it("takes a program of each open family no required program answers, the same at one seed (ADR 0916)", () => {
    const family = { idPrefix: "", code: "X", contexts: ["root"], variantPrefix: "v2", outcome: "false-clean", item: "x" };
    const picked = familyIds(rows, [], { families: [family], seed: 1 });
    expect(picked).toHaveLength(1);
    expect(["d", "f"].includes(picked[0]) && rows.find((one) => one.id === picked[0]).code).toBe("X");
    expect(familyIds(rows, [], { families: [family], seed: 1 })).toEqual(picked);
    expect(familyIds(rows, ["d"], { families: [family] })).toEqual([]);
    expect(familyIds(rows, [], { families: [{ ...family, code: "Z" }] })).toEqual([]);
    expect(sample({ grids: ["mx-recall"], codes: ["STRICT_READ_UNTRACKED"], families: [{ idPrefix: "mx-strict-read-untracked-", code: "STRICT_READ_UNTRACKED", contexts: ["ref-row"], variantPrefix: "store-dstr-", outcome: "false-clean", item: "x" }] }).byGrid["mx-recall"].some((id) => id.includes("-ref-row-"))).toBe(true);
  });
  it("gives each grid its share", () => {
    const { byGrid, programs } = sample({ grids: ["mx-base", "lg-base"], codes: ["STRICT_READ_UNTRACKED"] });
    expect(byGrid["mx-base"].length && byGrid["lg-base"].length).toBeGreaterThan(0);
    expect(byGrid["mx-base"].length + byGrid["lg-base"].length).toBe(programs);
  });
});
