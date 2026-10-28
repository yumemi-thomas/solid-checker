import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { compareDirs, diffRows, isUnexecutedMove, normalizeFindings, partitionRows } from "./compare-runs.mjs";

const row = (id, over = {}) => ({ id, seed: 1, verdict: "agree", reasons: [], items: [{ kind: "agree" }], checkerMs: 10, runtimeMs: 700, hits: { ran: 3 }, ...over });
const makeRun = (rows, { groups = [], repros = { abc: "x" } } = {}) => {
  const dir = mkdtempSync(join(tmpdir(), "fuzz-cmp-"));
  writeFileSync(join(dir, "results.jsonl"), rows.map((r) => JSON.stringify(r)).join("\n") + "\n");
  writeFileSync(join(dir, "summary.json"), JSON.stringify({ programs: rows.length, verdicts: {}, items: {}, candidates: 0, confirmed: 0, groups: groups.length }));
  writeFileSync(join(dir, "findings.json"), JSON.stringify({ generator: "g", seed: 1, count: rows.length, groups, refuted: [], falseClean: { groups: [{ repro: join(dir, "repros/abc"), signature: "s" }] } }));
  for (const [name, text] of Object.entries(repros)) { mkdirSync(join(dir, "repros", name), { recursive: true }); writeFileSync(join(dir, "repros", name, "App.tsx"), text); }
  return dir;
};

describe("comparing two runs", () => {
  it("ignores the wall times and nothing else of a row", () => {
    expect(diffRows([row("a")], [row("a", { checkerMs: 99, runtimeMs: 1 })])).toEqual([]);
    expect(diffRows([row("a")], [row("a", { verdict: "inconclusive" })])).toEqual([{ id: "a", fields: ["verdict"] }]);
    expect(diffRows([row("a")], [row("a", { hits: { ran: 4 } })])).toEqual([{ id: "a", fields: ["hits"] }]);
    expect(diffRows([row("a")], [])).toEqual([{ id: "a", fields: ["missing in B"] }]);
  });

  it("takes the output directory off the paths of findings.json", () => {
    expect(normalizeFindings({ repro: "/x/out/repros/abc", other: "/y" }, "/x/out")).toEqual({ repro: "<out>/repros/abc", other: "/y" });
  });

  it("calls two runs with the same rows, findings and minima equal, wherever they were written", () => {
    expect(compareDirs(makeRun([row("a")]), makeRun([row("a", { runtimeMs: 5 })]))).toEqual([]);
  });

  it("names a differing row, a differing minimum and a differing repro file", () => {
    expect(compareDirs(makeRun([row("a")]), makeRun([row("a", { verdict: "recall-gap" })]))).toEqual(["row a: verdict"]);
    expect(compareDirs(makeRun([row("a")]), makeRun([row("a")], { repros: { def: "x" } }))[0]).toMatch(/repros differ: only in A abc; only in B def/);
    expect(compareDirs(makeRun([row("a")]), makeRun([row("a")], { repros: { abc: "y" } }))).toEqual(["repro abc: files differ"]);
  });
});

describe("--unexecuted-moves: the one difference a later run may have", () => {
  const finding = { rule: "missing-owner", id: "SC4001", line: 9 };
  const never = (extra = {}) => ({ kind: "inconclusive", reason: "the site never ran", why: "never-ran", finding, ...extra });
  const candidate = (extra = {}) => ({ kind: "candidate-unexecuted", reason: "r", status: "candidate-false-positive: unexecuted", finding, siteIds: ["s1"], bases: ["dead-code"], ...extra });
  const before = row("a", { verdict: "inconclusive", reasons: ["the site never ran"], items: [never()] });
  const after = row("a", { verdict: "candidate-unexecuted", reasons: ["r"], items: [candidate()] });

  it("accepts an inconclusive: never-ran item that became a candidate-unexecuted one, and the verdict that follows", () => {
    expect(isUnexecutedMove(before, after)).toBe(true);
    expect(partitionRows([before], [after])).toEqual({ moves: ["a"], other: [] });
    // the program's verdict stays when a better tier outranks both
    const gap = { kind: "recall-gap", reason: "g", diagnostic: { code: "X" } };
    expect(isUnexecutedMove(row("a", { verdict: "recall-gap", items: [gap, never()] }), row("a", { verdict: "recall-gap", items: [gap, candidate()] }))).toBe(true);
  });

  it("refuses every other change, and a row that did not change", () => {
    expect(isUnexecutedMove(before, before)).toBe(false);
    expect(isUnexecutedMove(before, { ...after, hits: { ran: 9 } })).toBe(false);
    expect(isUnexecutedMove(before, row("a", { verdict: "candidate-false-positive", items: [{ kind: "candidate-false-positive", finding }] }))).toBe(false);
    expect(isUnexecutedMove(before, { ...after, items: [candidate({ finding: { ...finding, line: 10 } })] })).toBe(false);
    expect(isUnexecutedMove(before, { ...after, items: [candidate(), candidate()] })).toBe(false);
    expect(isUnexecutedMove(row("a", { verdict: "agree" }), after)).toBe(false);
    expect(isUnexecutedMove(row("a", { verdict: "inconclusive", items: [never({ why: "no-witness" })] }), after)).toBe(false);
    expect(partitionRows([before, row("b")], [after, row("b", { verdict: "recall-gap" })])).toEqual({ moves: ["a"], other: [{ id: "b", fields: ["verdict"] }] });
  });
});
