import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { isProved, ledgerText, namedIds, provedLines, provedMoves, provedOutcome, readLedger, record } from "./proved.mjs";
import { gridRows, sample } from "./cover.mjs";

// ADR 1012: a proved verdict never goes unnamed.
const ids24 = Array.from({ length: 24 }, (_, i) => `mx-store-write-outside-setter-lost-${String(i).padStart(2, "0")}`);
const rowsOf = (outcome, list = ids24) => list.map((id) => ({ id, outcome }));

describe("what is proved", () => {
  it("is agree on a misuse grid and clean on a legal one", () => {
    expect(provedOutcome("mx-census")).toBe("agree");
    expect(provedOutcome("lg-recall")).toBe("clean");
    expect(isProved("mx-recall", "agree")).toBe(true);
    for (const outcome of ["recall-gap", "not-fired", "typed-away", "harness", "false-clean", "stacked-claim"]) expect(isProved("mx-recall", outcome)).toBe(false);
    for (const outcome of ["uncertifiable", "typed-away", "unexecuted", "false-positive"]) expect(isProved("lg-recall", outcome)).toBe(false);
    expect(isProved("lg-recall", "agree")).toBe(false);
  });
});

describe("provedMoves", () => {
  const ledger = new Set(ids24);
  it("fails the 24 proved violations a merge lost unseen (#234): agree to recall-gap, none named", () => {
    // The laundering: ADR 0975-era merges turned 24 proved Vs into Us. focus.mjs read recall-gap as an outcome like another and the audit only counted.
    const moves = provedMoves({ grid: "mx-recall", ids: ledger, rows: rowsOf("recall-gap") });
    expect(moves.lost).toHaveLength(24);
    expect(moves.unnamed).toHaveLength(24);
    expect(provedLines("mx-recall", moves)).toHaveLength(11); // ten ids and "... 14 more"
    expect(provedLines("mx-recall", moves)[10]).toMatch(/14 more proved verdicts lost/);
  });
  it("naming 23 of the 24 still fails, naming all 24 with an ADR passes", () => {
    const changes = (list) => ({ rows: [{ grid: "mx-recall", ids: list, adr: "ADR 1012", why: "the fixture's merge" }] });
    const some = provedMoves({ grid: "mx-recall", ids: ledger, rows: rowsOf("recall-gap"), changes: changes(ids24.slice(0, 23)) });
    expect(some.unnamed.map((move) => move.id)).toEqual([ids24[23]]);
    expect(some.named).toBe(23);
    expect(provedMoves({ grid: "mx-recall", ids: ledger, rows: rowsOf("recall-gap"), changes: changes(ids24) }).unnamed).toEqual([]);
    // a row names ids of its own grid only
    expect(provedMoves({ grid: "mx-recall", ids: ledger, rows: rowsOf("recall-gap"), changes: { rows: [{ grid: "mx-census", ids: ids24, adr: "ADR 1012", why: "x" }] } }).unnamed).toHaveLength(24);
    expect(namedIds({ rows: [{ grid: "mx-recall", ids: ["a"] }, { grid: "mx-recall", count: 3 }] }, "mx-recall")).toEqual(new Set(["a"]));
  });
  it("fails a proved legal program that became uncertifiable (clean to U), and any other outcome away from proof", () => {
    const legal = ["lg-flush-a", "lg-flush-b"];
    expect(provedMoves({ grid: "lg-recall", ids: new Set(legal), rows: rowsOf("uncertifiable", legal) }).unnamed).toHaveLength(2);
    expect(provedMoves({ grid: "mx-recall", ids: ledger, rows: rowsOf("not-fired") }).lost[0]).toEqual({ id: ids24[0], to: "not-fired" });
    expect(provedMoves({ grid: "mx-recall", ids: ledger, rows: rowsOf("harness") }).unnamed).toHaveLength(24);
  });
  it("lets a move into proof through, and counts it", () => {
    const moves = provedMoves({ grid: "mx-recall", ids: new Set(), rows: rowsOf("agree") });
    expect(moves.lost).toEqual([]);
    expect(moves.gained).toBe(24);
    const lg = provedMoves({ grid: "lg-recall", ids: new Set(), rows: rowsOf("clean") });
    expect(lg.gained).toBe(24);
  });
  it("loses an id with no row only when the run was asked for every ledger id", () => {
    const rows = rowsOf("agree", ids24.slice(1));
    expect(provedMoves({ grid: "mx-recall", ids: ledger, rows }).lost).toEqual([]);
    expect(provedMoves({ grid: "mx-recall", ids: ledger, rows, expectAll: true }).lost).toEqual([{ id: ids24[0], to: "no row" }]);
  });
});

describe("the ledger files", () => {
  it("record adds the proved ids of a run, sorted, and never removes one", () => {
    const run = mkdtempSync(join(tmpdir(), "proved-run-"));
    const dir = mkdtempSync(join(tmpdir(), "proved-ledger-"));
    const write = (grid, rows) => { mkdirSync(join(run, grid), { recursive: true }); writeFileSync(join(run, grid, "programs.jsonl"), rows.map((row) => JSON.stringify(row)).join("\n")); };
    write("mx-recall", [{ id: "b", outcome: "agree" }, { id: "a", outcome: "agree" }, { id: "c", outcome: "recall-gap" }]);
    write("lg-recall", [{ id: "l1", outcome: "clean" }, { id: "l2", outcome: "uncertifiable" }]);
    expect(record(run, dir)).toEqual({ "lg-recall": { before: 0, after: 1 }, "mx-recall": { before: 0, after: 2 } });
    expect(readFileSync(join(dir, "mx-recall.json"), "utf8")).toBe('{"grid":"mx-recall","ids":[\n"a",\n"b"\n]}\n');
    // a later run in which "a" is lost and "z" proved: the ledger keeps "a" (a loss is named in changes.json, never forgotten) and adds "z"
    write("mx-recall", [{ id: "a", outcome: "recall-gap" }, { id: "z", outcome: "agree" }]);
    expect(record(run, dir)["mx-recall"]).toEqual({ before: 2, after: 3 });
    expect(readLedger(dir).get("mx-recall")).toEqual(new Set(["a", "b", "z"]));
    expect(ledgerText("g", ["b", "a", "b"])).toBe('{"grid":"g","ids":[\n"a",\n"b"\n]}\n');
  });
});

describe("the cover holds the ledger (ADR 1012)", () => {
  const codes = ["STRICT_READ_UNTRACKED"];
  const grids = ["mx-base", "lg-base"];
  it("adds a ledger id the greedy cover left out, keeps the greedy cover itself, and adds nothing for a ledger drawn from it", () => {
    const plain = sample({ grids, codes, proved: new Map() });
    const chosen = new Set(Object.values(plain.byGrid).flat());
    const left = gridRows("mx-base", { codes }).map((row) => row.id).find((id) => !chosen.has(id));
    expect(left).toBeTruthy();
    const withLedger = sample({ grids, codes, proved: new Map([["mx-base", new Set([left])]]) });
    expect(withLedger.byGrid["mx-base"]).toEqual([...plain.byGrid["mx-base"], left]);
    expect(withLedger.byGrid["lg-base"]).toEqual(plain.byGrid["lg-base"]);
    const drawn = new Map(Object.entries(plain.byGrid).map(([grid, ids]) => [grid, new Set(ids)]));
    expect(sample({ grids, codes, proved: drawn }).byGrid).toEqual(plain.byGrid);
    // an id the grid does not generate cannot be added: the smoke then reports it lost (no row)
    expect(sample({ grids, codes, proved: new Map([["mx-base", new Set(["mx-not-a-program"])]]) }).byGrid["mx-base"]).toEqual(plain.byGrid["mx-base"]);
  });
  it("leaves the per-code tier alone", () => {
    const perCode = sample({ perCode: true, grids, codes, proved: new Map([["mx-base", new Set(["x"])]]) });
    expect(perCode.byGrid["mx-base"]).not.toContain("x");
  });
});
