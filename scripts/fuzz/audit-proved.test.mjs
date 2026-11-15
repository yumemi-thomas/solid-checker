import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { applyKnown, auditProved, countFor, itemOf, readAuditLedger, writeAuditLedger } from "./audit-proved.mjs";
import { readOracle } from "./audit.mjs";

// ADR 1012: the audit holds a run to the ledger of the last accepted audit, and takes the known items out of "needs a look".
const ids = Array.from({ length: 24 }, (_, i) => `mx-store-write-outside-setter-lost-${String(i).padStart(2, "0")}`);
const CODE = "STORE_WRITE_OUTSIDE_SETTER";
const ledgerOf = (grid = "mx-recall", list = ids, code = CODE, consumed = []) => ({ grids: new Map([[grid, new Map(list.map((id) => [id, code]))]]), consumed: new Set(consumed) });
const rowsOf = (outcome, grid = "mx-recall", list = ids) => new Map([[grid, new Map(list.map((id) => [id, outcome]))]]);
const group = (over = {}) => ({ grid: "mx-recall", code: CODE, from: "agree", to: "recall-gap", count: 24, adr: "ADR 1012", why: "the fixture's merge", ...over });

describe("the audit's proved-verdict check", () => {
  it("fails the 24 proved violations a merge lost unseen (#234) and does not accept the run as the next baseline", () => {
    // The laundering: the audit passed the last report on as the baseline, green or not, and only counted the move.
    const check = auditProved({ rows: rowsOf("recall-gap"), codes: new Map(), ledger: ledgerOf() });
    expect(check.accepted).toBe(false);
    expect(check.losses).toBe(24);
    expect(check.attention).toHaveLength(1);
    expect(check.attention[0]).toMatch(/mx-recall: 24 proved verdict\(s\) of STORE_WRITE_OUTSIDE_SETTER lost \(agree -> recall-gap\), not named/);
  });
  it("naming 23 of 24 still fails, naming 24 is accepted, and a row names its losses once", () => {
    const run = (rows, consumed = []) => auditProved({ rows: rowsOf("recall-gap"), codes: new Map(), ledger: ledgerOf("mx-recall", ids, CODE, consumed), changes: { rows } });
    expect(run([group({ count: 23 })]).attention[0]).toMatch(/1 proved verdict\(s\)/);
    const accepted = run([group()]);
    expect(accepted.accepted).toBe(true);
    expect(accepted.attention).toEqual([]);
    expect(accepted.consumed.size).toBe(1);
    // the next audit (the ledger's `consumed` now holds the row): a later loss of the same group is not named by it
    const [digest] = accepted.consumed;
    expect(run([group()], [digest]).accepted).toBe(false);
    // two rows add up; a row of another code or outcome names nothing
    expect(run([group({ count: 20 }), group({ count: 4, why: "the rest" })]).accepted).toBe(true);
    expect(run([group({ code: "OTHER" })]).accepted).toBe(false);
    expect(run([group({ to: "not-fired" })]).accepted).toBe(false);
    expect(run([group({ grid: "mx-census" })]).accepted).toBe(false);
  });
  it("names exact ids too, and counts a program the run no longer generates as lost (gone)", () => {
    const some = auditProved({ rows: rowsOf("recall-gap"), codes: new Map(), ledger: ledgerOf(), changes: { rows: [{ grid: "mx-recall", ids: ids.slice(0, 20), adr: "ADR 1012", why: "x" }] } });
    expect(some.accepted).toBe(false);
    expect(some.attention[0]).toMatch(/4 proved verdict/);
    const gone = auditProved({ rows: new Map([["mx-recall", new Map()]]), codes: new Map(), ledger: ledgerOf() });
    expect(gone.attention[0]).toMatch(/\(agree -> gone\)/);
    const none = auditProved({ rows: new Map(), codes: new Map(), ledger: ledgerOf() });
    expect(none.accepted).toBe(false);
    expect(none.attention[0]).toMatch(/judged no program of it/);
  });
  it("holds a legal grid to clean, lets moves into proof through, and needs a ledger", () => {
    const legal = ["lg-a", "lg-b"];
    const lost = auditProved({ rows: rowsOf("uncertifiable", "lg-recall", legal), codes: new Map(), ledger: ledgerOf("lg-recall", legal, "FLUSH_IN_ACTION") });
    expect(lost.attention[0]).toMatch(/lg-recall: 2 proved verdict\(s\) of FLUSH_IN_ACTION lost \(clean -> uncertifiable\)/);
    const into = auditProved({ rows: rowsOf("agree", "mx-recall", [...ids, "mx-new"]), codes: new Map(), ledger: ledgerOf() });
    expect(into).toMatchObject({ accepted: true, gained: 1, losses: 0 });
    expect(auditProved({ rows: rowsOf("agree"), codes: new Map(), ledger: null }).accepted).toBe(false);
    expect(auditProved({ rows: rowsOf("agree"), codes: new Map(), ledger: null }).attention[0]).toMatch(/no proved ledger/);
    expect(auditProved({ rows: rowsOf("agree"), codes: new Map(), ledger: null, bootstrap: true })).toMatchObject({ accepted: true, attention: [] });
  });
  it("writes the ledger as id and code per proved program, and reads it back with the consumed rows", () => {
    const dir = mkdtempSync(join(tmpdir(), "audit-ledger-"));
    const rows = new Map([["mx-recall", new Map([["b", "agree"], ["a", "agree"], ["c", "recall-gap"]])], ["lg-recall", new Map([["l", "clean"], ["m", "uncertifiable"]])]]);
    const codes = new Map([["mx-recall", new Map([["a", "CA"], ["b", "CB"]])], ["lg-recall", new Map([["l", "CL"]])]]);
    writeAuditLedger(dir, rows, codes, new Set(["d1", "d0"]));
    const ledger = readAuditLedger(dir);
    expect([...ledger.grids.get("mx-recall")]).toEqual([["a", "CA"], ["b", "CB"]]);
    expect([...ledger.grids.get("lg-recall")]).toEqual([["l", "CL"]]);
    expect(ledger.consumed).toEqual(new Set(["d0", "d1"]));
    expect(readAuditLedger(join(dir, "none"))).toBeNull();
  });
});

describe("the audit's known items", () => {
  const attention = [
    ...Array.from({ length: 3 }, (_, i) => `mx-recall: unplaced mx-strict-read-untracked-root-memo-direct-none-pk-lazy-${i} {"placed":"unplaced"}`),
    "mx-recall: halted-claim mx-settled-cleanup-unowned-flw-flow-for-async-x {\"reported\":[]}",
    "mx-recall: oracle inconclusive mx-reactive-write-patch-a: Browser feedback bootstrap did not bind",
    "mx-recall: oracle inconclusive mx-reactive-write-patch-b: page.waitForFunction: Timeout 12000ms exceeded.",
    "mx-census: oracle differs mx-census-x store-write: timing",
    "mx-recall: 3 programs judged of 4",
  ];
  const known = (over = []) => ({ entries: [
    { grid: "mx-recall", kind: "unplaced", item: "pk-lazy", idRegex: "-pk-lazy-", count: 3, adr: "ADR 1012", why: "x" },
    { grid: "mx-recall", kind: "oracle inconclusive", item: "timeouts", idRegex: "^mx-", count: 5, cap: true, adr: "ADR 1012", why: "x" },
    ...over,
  ] });
  it("parses an attention line's grid, kind and id", () => {
    expect(itemOf(attention[0])).toEqual({ grid: "mx-recall", kind: "unplaced", id: "mx-strict-read-untracked-root-memo-direct-none-pk-lazy-0" });
    expect(itemOf(attention[4])).toEqual({ grid: "mx-recall", kind: "oracle inconclusive", id: "mx-reactive-write-patch-a" });
    expect(itemOf(attention[7])).toBeNull();
  });
  it("takes the known items out, keeps the new ones, and shows the counts", () => {
    const applied = applyKnown(attention, known());
    expect(applied.attention).toEqual([attention[3], attention[6], attention[7]]);
    expect(applied.known).toEqual([
      { item: "pk-lazy", grid: "mx-recall", kind: "unplaced", known: 3, count: 3, cap: false },
      { item: "timeouts", grid: "mx-recall", kind: "oracle inconclusive", known: 2, count: 5, cap: true },
    ]);
  });
  it("a class that rises stands out: the extra ones stay in attention, with a line that names them", () => {
    const more = [...attention, "mx-recall: unplaced mx-store-write-outside-setter-pk-lazy-9 {}"];
    const applied = applyKnown(more, known());
    expect(applied.attention.some((line) => /known item mx-recall unplaced pk-lazy rose: 4 now, 3 known/.test(line))).toBe(true);
    expect(applied.attention.filter((line) => line.startsWith("mx-recall: unplaced"))).toHaveLength(1);
    // a cap above its count: attention
    const flaky = [...attention, ...Array.from({ length: 4 }, (_, i) => `mx-recall: oracle inconclusive mx-extra-${i}: timeout`)];
    expect(applyKnown(flaky, known()).attention.some((line) => /oracle inconclusive timeouts rose: 6 now, 5 known/.test(line))).toBe(true);
  });
  it("a class that falls below an exact count is attention (a fix is news); a cap may fall", () => {
    const fewer = attention.filter((line) => !line.includes("pk-lazy-2") && !line.includes("patch-b"));
    const applied = applyKnown(fewer, known());
    expect(applied.attention.some((line) => /known item mx-recall unplaced pk-lazy fell: 2 now, 3 known: lower its count/.test(line))).toBe(true);
    expect(applied.attention.some((line) => /timeouts/.test(line))).toBe(false);
  });
  it("an entry's count may differ by recall mode: the lean audit holds the lean number", () => {
    const halted = Array.from({ length: 5 }, (_, i) => `mx-recall: halted-claim mx-settled-flw-flow-for-async-${i} {}`);
    const byMode = { entries: [{ grid: "mx-recall", kind: "halted-claim", item: "flw", idRegex: "flw-flow-for-async", count: { lean: 3, full: 5 }, adr: "ADR 1012", why: "x" }] };
    expect(applyKnown(halted, byMode, "full").attention).toEqual([]);
    expect(applyKnown(halted.slice(0, 3), byMode, "lean").attention).toEqual([]);
    expect(applyKnown(halted, byMode, "lean").attention.some((line) => /rose: 5 now, 3 known/.test(line))).toBe(true);
    expect(countFor({ count: 7 }, "lean")).toBe(7);
    expect(countFor({ count: { lean: 1 } }, "full")).toBe(0);
  });
  it("matches by grid, kind and id only: a halted claim is not an unplaced one, and an unknown grid keeps its lines", () => {
    const applied = applyKnown(["mx-census: unplaced mx-strict-pk-lazy-1 {}"], known());
    expect(applied.attention.filter((line) => line.startsWith("mx-census"))).toHaveLength(1);
  });
});

describe("the oracle rerun", () => {
  it("replaces an inconclusive program's result by its second run, and counts only what stays inconclusive", () => {
    const dir = mkdtempSync(join(tmpdir(), "audit-oracle-"));
    const part = join(dir, "mx-recall");
    mkdirSync(join(part, "rerun"), { recursive: true });
    const write = (path, value) => writeFileSync(path, typeof value === "string" ? value : `${JSON.stringify(value)}\n`);
    write(join(part, "runtime-diff.json"), { summary: { compared: 3, inconclusive: 2, same: 1, differ: 0, behaviour: 0, attributionOnly: 0, byKind: {} }, differing: [] });
    write(join(part, "runtime-diff.jsonl"), [{ id: "a" }, { id: "b", inconclusive: "timeout" }, { id: "c", inconclusive: "bootstrap" }].map((row) => JSON.stringify(row)).join("\n"));
    write(join(part, "programs.jsonl"), "");
    write(join(part, "rerun", "runtime-diff.json"), { summary: { compared: 2 }, differing: [] });
    write(join(part, "rerun", "runtime-diff.jsonl"), [{ id: "b" }, { id: "c", inconclusive: "bootstrap again" }].map((row) => JSON.stringify(row)).join("\n"));
    const { oracleRows, diff } = readOracle(dir, "mx-recall", { ids: ["a", "b", "c"] });
    expect(oracleRows.filter((row) => row.inconclusive).map((row) => `${row.id}:${row.inconclusive}`)).toEqual(["c:bootstrap again"]);
    expect(oracleRows.map((row) => row.id).sort()).toEqual(["a", "b", "c"]);
    expect(diff.summary.inconclusive).toBe(1);
  });
});
