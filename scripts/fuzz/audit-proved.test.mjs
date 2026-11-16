import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { applyKnown, auditProved, itemOf, readAuditLedger, writeAuditLedger } from "./audit-proved.mjs";
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
  const unplaced = (i) => `mx-recall: unplaced mx-strict-read-untracked-root-memo-direct-none-pk-lazy-${i} {"placed":"unplaced"}`;
  const attention = [
    ...Array.from({ length: 3 }, (_, i) => unplaced(i)),
    "mx-recall: halted-claim mx-settled-cleanup-unowned-flw-flow-for-async-x {\"reported\":[]}",
    "mx-recall: oracle inconclusive mx-reactive-write-patch-a: Browser feedback bootstrap did not bind",
    "mx-recall: oracle inconclusive mx-reactive-write-patch-b: page.waitForFunction: Timeout 12000ms exceeded.",
    "mx-census: oracle differs mx-census-x store-write: timing",
    "mx-recall: 3 programs judged of 4",
  ];
  const lazy = (list) => list.map((i) => `mx-strict-read-untracked-root-memo-direct-none-pk-lazy-${i}`);
  const known = (over = {}) => ({ entries: [
    { grid: "mx-recall", kind: "unplaced", item: "pk-lazy", idRegex: "-pk-lazy-", ids: { lean: lazy([0, 1, 2]), full: lazy([0, 1, 2]) }, adr: "ADR 1012", why: "x" },
    { grid: "mx-recall", kind: "oracle inconclusive", item: "timeouts", idRegex: "^mx-", cap: true, ids: { lean: ["mx-reactive-write-patch-a", "mx-reactive-write-patch-b"], full: ["mx-reactive-write-patch-a", "mx-reactive-write-patch-b", "mx-other"] }, adr: "ADR 1012", why: "x" },
    ...(over.entries ?? []),
  ] });
  it("parses an attention line's grid, kind and id", () => {
    expect(itemOf(attention[0])).toEqual({ grid: "mx-recall", kind: "unplaced", id: "mx-strict-read-untracked-root-memo-direct-none-pk-lazy-0" });
    expect(itemOf(attention[4])).toEqual({ grid: "mx-recall", kind: "oracle inconclusive", id: "mx-reactive-write-patch-a" });
    expect(itemOf(attention[7])).toBeNull();
  });
  it("takes the stored ids out, keeps the new ones, and shows the counts", () => {
    const applied = applyKnown(attention, known());
    expect(applied.attention).toEqual([attention[3], attention[6], attention[7]]);
    expect(applied.known).toEqual([
      { item: "pk-lazy", grid: "mx-recall", kind: "unplaced", known: 3, count: 3, cap: false },
      { item: "timeouts", grid: "mx-recall", kind: "oracle inconclusive", known: 2, count: 3, cap: true },
    ]);
  });
  it("a swap shows: one known id fixed and a new one of the same class is attention both ways (a count alone would pass)", () => {
    // The laundering: pk-lazy-2 is fixed and a NEW program, pk-lazy-9, ends unplaced: the class still counts 3.
    const swapped = [unplaced(0), unplaced(1), unplaced(9), ...attention.slice(3)];
    const applied = applyKnown(swapped, known());
    expect(applied.attention.some((line) => /known item mx-recall unplaced pk-lazy: 1 id\(s\) it does not store \(3 stored\): .*pk-lazy-9/.test(line))).toBe(true);
    expect(applied.attention.some((line) => /1 stored id\(s\) no longer reported .*pk-lazy-2/.test(line))).toBe(true);
    expect(applied.attention.filter((line) => line.startsWith("mx-recall: unplaced"))).toHaveLength(1);
  });
  it("a class that rises stands out: the ids it does not store stay in attention", () => {
    const more = [...attention, "mx-recall: unplaced mx-store-write-outside-setter-pk-lazy-9 {}"];
    const applied = applyKnown(more, known());
    expect(applied.attention.some((line) => /known item mx-recall unplaced pk-lazy: 1 id\(s\) it does not store/.test(line))).toBe(true);
    expect(applied.attention.filter((line) => line.startsWith("mx-recall: unplaced"))).toHaveLength(1);
  });
  it("a cap entry accepts a subset of its ids and nothing else; an exact entry needs every stored id", () => {
    // a subset: fine, even for the id the run did not report
    const subset = attention.filter((line) => !line.includes("patch-b"));
    expect(applyKnown(subset, known()).attention.some((line) => /timeouts/.test(line))).toBe(false);
    // an id outside the stored set: attention, cap or not
    const flaky = [...attention, "mx-recall: oracle inconclusive mx-new-flake: timeout"];
    expect(applyKnown(flaky, known()).attention.some((line) => /oracle inconclusive timeouts: 1 id\(s\) it does not store/.test(line))).toBe(true);
    // a stored id of an exact entry that is no longer reported: a fix is news
    const fewer = attention.filter((line) => !line.includes("pk-lazy-2"));
    expect(applyKnown(fewer, known()).attention.some((line) => /pk-lazy: 1 stored id\(s\) no longer reported/.test(line))).toBe(true);
  });
  it("an entry's ids differ by recall mode: the lean audit holds the lean ids", () => {
    const halted = Array.from({ length: 5 }, (_, i) => `mx-recall: halted-claim mx-settled-flw-flow-for-async-${i} {}`);
    const id = (i) => `mx-settled-flw-flow-for-async-${i}`;
    const byMode = { entries: [{ grid: "mx-recall", kind: "halted-claim", item: "flw", idRegex: "flw-flow-for-async", ids: { lean: [0, 1, 2].map(id), full: [0, 1, 2, 3, 4].map(id) }, adr: "ADR 1012", why: "x" }] };
    expect(applyKnown(halted, byMode, "full").attention).toEqual([]);
    expect(applyKnown(halted.slice(0, 3), byMode, "lean").attention).toEqual([]);
    expect(applyKnown(halted, byMode, "lean").attention.some((line) => /2 id\(s\) it does not store \(3 stored\)/.test(line))).toBe(true);
    expect(applyKnown(halted, byMode, "mixed").attention.some((line) => /5 id\(s\) it does not store \(0 stored\)/.test(line))).toBe(true);
  });
  it("matches by grid, kind and id only: a halted claim is not an unplaced one, and an unknown grid keeps its lines", () => {
    const applied = applyKnown(["mx-census: unplaced mx-strict-pk-lazy-1 {}"], known());
    expect(applied.attention.filter((line) => line.startsWith("mx-census"))).toHaveLength(1);
  });
  it("the committed audit-known.json stores sorted, distinct ids for both modes, and its lean ids are in the full ones", () => {
    const file = JSON.parse(readFileSync(new URL("./audit-known.json", import.meta.url), "utf8"));
    for (const entry of file.entries) {
      for (const mode of ["lean", "full"]) {
        const ids = entry.ids?.[mode];
        expect(Array.isArray(ids) && ids.length > 0, `${entry.item} ${entry.grid} ${mode}`).toBe(true);
        expect(new Set(ids).size).toBe(ids.length);
        expect([...ids].sort()).toEqual(ids);
      }
      expect(entry.ids.lean.every((id) => entry.ids.full.includes(id))).toBe(true);
    }
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

describe("the bootstrap and the ledger that should exist", () => {
  it("a bootstrap spends every changes row that exists, so none names a loss after it", () => {
    // The laundering: rows written before the first baseline sat unspent, and named the first losses after it.
    const rows = [group(), group({ code: "OTHER", why: "another" })];
    const boot = auditProved({ rows: rowsOf("agree"), codes: new Map(), ledger: null, changes: { rows }, bootstrap: true });
    expect(boot.accepted).toBe(true);
    expect(boot.consumed.size).toBe(2);
    // the next audit reads that ledger: the rows are consumed, so the 24 losses are unnamed
    const next = auditProved({ rows: rowsOf("recall-gap"), codes: new Map(), ledger: ledgerOf("mx-recall", ids, CODE, [...boot.consumed]), changes: { rows } });
    expect(next.accepted).toBe(false);
    expect(next.attention[0]).toMatch(/24 proved verdict\(s\)/);
  });
  it("a ledger the workflow found and the run could not read is attention, never `no ledger`", () => {
    const lost = auditProved({ rows: rowsOf("agree"), codes: new Map(), ledger: null, expected: true });
    expect(lost.accepted).toBe(false);
    expect(lost.attention[0]).toMatch(/main has one .* could not be read/);
    // not a bootstrap either: expected wins
    expect(auditProved({ rows: rowsOf("agree"), codes: new Map(), ledger: null, expected: true, bootstrap: true }).accepted).toBe(false);
  });
});

describe("the audit workflow's proved ledger", () => {
  const workflow = readFileSync(new URL("../../.github/workflows/fuzz-audit.yml", import.meta.url), "utf8");
  const step = (name) => workflow.slice(workflow.indexOf(name), workflow.indexOf("\n      - ", workflow.indexOf(name) + 1));
  it("finds the ledger by the artifact's name over the repository, not in a window of runs, and counts every one that ever existed", () => {
    const found = step("- id: proved\n        env:");
    expect(found).toMatch(/actions\/artifacts\?name=audit-proved-\$RECALL/);
    expect(found).toMatch(/total_count/);
    expect(found).not.toMatch(/gh run list/);
    expect(workflow).toMatch(/proved-ever: \$\{\{ steps\.proved\.outputs\.ever \}\}/);
  });
  it("refuses a bootstrap when an audit-proved artifact of the mode ever existed, and fails the run when the ledger cannot be downloaded", () => {
    expect(workflow).toMatch(/\[ "\$\{PROVED_EVER:-0\}" != 0 \]/);
    expect(workflow).toMatch(/proved-bootstrap refused/);
    expect(workflow).toMatch(/proved\+=\(--proved-expected true\)/);
    const download = workflow.slice(workflow.indexOf("# A ledger that exists and cannot be downloaded"), workflow.indexOf("- name: Report"));
    expect(download).toMatch(/audit-proved-\$\{\{ needs\.build\.outputs\.recall \}\}/);
    expect(download).not.toMatch(/continue-on-error/);
  });
  it("uploads the oracle rerun's results with the oracle artifact", () => {
    const upload = workflow.slice(workflow.indexOf("name: oracle-${{ matrix.grid }}-part${{ matrix.part }}"));
    expect(upload.slice(0, 600)).toMatch(/oracle\/\$\{\{ matrix\.grid \}\}\/rerun\/\*\.json\n.*oracle\/\$\{\{ matrix\.grid \}\}\/rerun\/\*\.jsonl/);
  });
});
