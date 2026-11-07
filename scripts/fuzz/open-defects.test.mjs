import { existsSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { finalBlockers, gridStanding, heldGridEntry, ledgerCounts, ledgerCovers, namedKey, offLedger, ownerProblems, ratchetDifferences, readOpenDefects } from "./open-defects.mjs";
import { inStopGroup, ledgerChange, runSeed, stopFinding } from "./open-rows.mjs";
import { NO_RUNTIME_WARNING } from "./checker.mjs";
import { PATTERNS, comparePatterns, expectedSilent, judgePattern, patternCases } from "./patterns.mjs";

const finding = (rule, kind) => ({ rule, kind, primaryLocation: { path: "App.tsx" } });

describe("expected-silent.json (ADR 0696)", () => {
  it("passes a listed program while its rule stays reported, and fails it when the rule goes", () => {
    const rules = ["no-store-mutation-outside-setter"];
    expect(expectedSilent({ outcome: "false-positive", rules }, rules).outcome).toBe("fine");
    expect(expectedSilent({ outcome: "fine", codes: [] }, rules).outcome).toBe("false-clean");
    expect(expectedSilent({ outcome: "false-positive", rules: ["other"] }, undefined).outcome).toBe("false-positive");
  });
});

describe("the known open list", () => {
  const list = readOpenDefects();
  it("says how much is open, and names a queue item and a program for each", () => {
    const count = (outcome) => list.patterns.filter((entry) => entry.outcome === outcome).length;
    console.log(`known open: ${list.patterns.length} patterns (${count("false-positive")} false positive, ${count("false-clean")} false clean), ${list.grids.length} grid families`);
    for (const entry of list.patterns) {
      expect(entry.item).toBeTruthy();
      expect(["false-positive", "false-clean", "misplaced", "no-run", "stacked-claim"]).toContain(entry.outcome);
      expect(existsSync(join(PATTERNS, entry.id, "App.tsx")), entry.id).toBe(true);
    }
  });
  it("a listed program that passes fails the run, and so does a failure nobody listed", () => {
    const entries = { patterns: [{ id: "a/x", outcome: "false-clean" }, { id: "a/y", outcome: "false-clean" }] };
    const { held, unlisted, fixed } = comparePatterns([{ id: "a/x", outcome: "false-clean" }, { id: "a/y", outcome: "fine" }, { id: "a/z", outcome: "false-positive" }], entries);
    expect(held.map(({ row }) => row.id)).toEqual(["a/x"]);
    expect(fixed.map((row) => row.id)).toEqual(["a/y"]);
    expect(unlisted.map((row) => row.id)).toEqual(["a/z"]);
  });
  it("has the programs on disk", () => {
    expect(patternCases().length).toBeGreaterThan(600);
  });
});

describe("the pattern judge", () => {
  it("a code nothing covers is a false clean, certified with no obligation in the program", () => {
    expect(judgePattern({ codes: ["STRICT_READ_UNTRACKED"], positions: false }, [])).toMatchObject({ outcome: "false-clean", certified: true });
    expect(judgePattern({ codes: ["STRICT_READ_UNTRACKED"], stacks: { STRICT_READ_UNTRACKED: [[{ file: "App.tsx", line: 4, column: 2 }]] } }, [])).toMatchObject({ outcome: "false-clean", certified: true });
    // With no recorded position (R8), unjudged: `unplaced`, never a pass or a false clean.
    expect(judgePattern({ codes: ["STRICT_READ_UNTRACKED"] }, []).outcome).toBe("unplaced");
  });
  it("a violation Chrome did not report is a false positive", () => {
    expect(judgePattern({ codes: [] }, [finding("strict-read-untracked", "violation")]).outcome).toBe("false-positive");
  });
  it("an obligation answers a code, and a failed page is no verdict", () => {
    expect(judgePattern({ codes: ["STRICT_READ_UNTRACKED"], stacks: { STRICT_READ_UNTRACKED: [[{ file: "App.tsx", line: 4, column: 2 }]] } }, [{ ...finding("strict-read-untracked", "uncertifiable"), primaryLocation: { path: "App.tsx", line: 4, column: 2 } }]).outcome).toBe("fine");
    expect(judgePattern({ codes: [], failure: "x" }, []).outcome).toBe("no-run");
    // sc1001.md step 0: the checker says no window is open, and the lane measured the quiet.
    expect(judgePattern({ codes: [] }, [{ ...finding("strict-read-untracked", "violation"), hint: `${NO_RUNTIME_WARNING} here.` }]).outcome).toBe("fine");
  });
  // Review 4c, R5, R8: an SC9012, or a claiming row of a code with a place, covers it only at a position Chrome recorded for it: the top
  // frame's, or a deeper frame's exact call position on a line that mounts nothing. A placeless code: a claiming row anywhere, never an SC9012.
  it("a withholding or a claiming row covers a code only at a recorded position", () => {
    const at = (rule, line, column, path = "/w/p/App.tsx", endLine = line) => ({ id: "SC", rule, kind: "uncertifiable", analysisContext: "value-forwarded", primaryLocation: { path, line, column, endLine } });
    const withheld = (line, column = 5, path, endLine) => at("reactive-dispatch-unresolved", line, column, path, endLine);
    const sru = (...stack) => ({ codes: ["STRICT_READ_UNTRACKED"], stacks: { STRICT_READ_UNTRACKED: [stack] } });
    const top = sru({ file: "App.tsx", line: 7, column: 5 });
    expect(judgePattern(top, [withheld(7)]).outcome).toBe("fine");
    expect(judgePattern(top, [withheld(7, 6)]).outcome).toBe("false-clean");
    expect(judgePattern(top, [withheld(9)])).toMatchObject({ outcome: "false-clean", certified: false });
    expect(judgePattern(top, [withheld(7, 5, "/w/p/h.ts")]).outcome).toBe("false-clean");
    expect(judgePattern(top, [withheld(6, 1, undefined, 12)]).outcome).toBe("false-clean");
    expect(judgePattern({ codes: ["STRICT_READ_UNTRACKED"] }, [withheld(7)]).outcome).toBe("unplaced");
    expect(judgePattern(top, [at("strict-read-untracked", 7, 5)]).outcome).toBe("fine");
    expect(judgePattern(top, [{ ...at("strict-read-untracked", 12, 5), kind: "violation" }]).outcome).toBe("misplaced");
    expect(judgePattern(top, [at("strict-read-untracked", 12, 5)]).outcome).toBe("false-clean");
    // The helper's call in App, the frame below the read, leads to it; a mount line leads to everything.
    expect(judgePattern(sru({ file: "h.ts", line: 2, column: 3 }, { file: "App.tsx", line: 9, column: 5 }), [withheld(9)]).outcome).toBe("fine");
    expect(judgePattern(sru({ file: "h.ts", line: 2, column: 3 }, { file: "App.tsx", line: 9, column: 5, mount: true }), [withheld(9)]).outcome).toBe("false-clean");
    expect(judgePattern({ codes: ["ASYNC_OUTSIDE_LOADING_BOUNDARY"] }, [withheld(9)]).outcome).toBe("false-clean");
    expect(judgePattern({ codes: ["ASYNC_OUTSIDE_LOADING_BOUNDARY"] }, [at("async-outside-loading-boundary", 9, 1)]).outcome).toBe("fine");
  });
});

describe("the row ledger (ADR 0815, R23)", () => {
  const row = { id: "mx-a-1", code: "C", context: "c", outcome: "false-clean", detail: {} };
  const list = { grids: [{ idPrefix: "mx-a-", code: "C", outcome: "false-clean", item: "fam" }, { idPrefix: "mx-a-", code: "C", outcome: "misplaced", item: "other" }], rows: { seed: 7, grids: { g: { "mx-a-1": { outcome: "false-clean", cause: "R" } } } } };
  it("holds a listed row, and only with its own outcome", () => {
    expect(gridStanding(list, row, "false-clean", { grid: "g" }).kind).toBe("held");
    expect(gridStanding(list, row, "misplaced", { grid: "g" }).kind).toBe("moved");
  });
  it("fails a row turned certified", () => {
    expect(gridStanding(list, { ...row, detail: { certified: true } }, "false-clean", { grid: "g" }).kind).toBe("certified");
  });
  it("fails a failing row off the ledger where the ledger covers the run, and only prints it elsewhere", () => {
    const other = { ...row, id: "mx-a-2" };
    expect(gridStanding(list, other, "false-clean", { grid: "g", strict: true })).toBe(null);
    expect(gridStanding(list, other, "false-clean", { grid: "g" }).kind).toBe("unlisted");
    expect(ledgerCovers(list, 7)).toBe(true);
    expect(ledgerCovers(list, 8)).toBe(false);
    expect(ledgerCovers({ rows: { complete: true } }, 8)).toBe(true);
  });
  it("a listed row that stopped failing fails the run; a passing row off the ledger is nothing", () => {
    expect(gridStanding(list, { ...row, outcome: "agree" }, null, { grid: "g" }).kind).toBe("fixed");
    expect(gridStanding(list, { ...row, id: "mx-a-2", outcome: "agree" }, null, { grid: "g" })).toBe(null);
  });
  it("the final gate fails while R-F1-open or R-F2-open holds a row, or any certified or false-positive row is listed (before #173)", () => {
    const rows = { grids: { g: { a: { outcome: "false-clean", certified: true, cause: "R-F2-open" }, b: { outcome: "false-clean", certified: true, cause: "R" } } } };
    expect(finalBlockers({ rows })).toEqual(["g/a", "g/b"]);
    expect(finalBlockers({ rows: { grids: { g: { b: rows.grids.g.b } } } })).toEqual(["g/b"]);
    expect(finalBlockers({ rows: { grids: { g: { c: { outcome: "false-clean", cause: "R-F1-open" } } } } })).toEqual(["g/c"]);
    expect(finalBlockers({ rows: { grids: { g: { c: { outcome: "false-clean", cause: "R-alias-owner" } } } } })).toEqual(["g/c"]);
    expect(finalBlockers({ rows: { grids: { g: { c: { outcome: "secondary-false-clean", cause: "R-casts-stage2" } } } } })).toEqual(["g/c"]);
    expect(finalBlockers({ rows: { grids: { g: { d: { outcome: "false-positive", cause: "fam" } } } } })).toEqual(["g/d"]);
    expect(finalBlockers({ rows: { grids: {} }, patterns: [{ id: "p/a", outcome: "false-positive" }, { id: "p/b", outcome: "false-clean" }, { id: "p/c", outcome: "false-clean", certified: true }] })).toEqual(["patterns/p/a", "patterns/p/c"]);
    expect(finalBlockers({ rows: { grids: { g: { e: { outcome: "false-clean", cause: "R" }, f: { outcome: "misplaced", cause: "fam" } } } } })).toEqual([]);
    expect(ledgerCounts(rows)).toMatchObject({ certified: 1, certifiedF2: 1 });
    if (process.env.FLIP_FINAL) expect(finalBlockers(readOpenDefects())).toEqual([]);
  });
  it("a certified or false-positive row with no family has an owner, and an owner names a listed row with that owner", () => {
    const rows = { grids: { g: { a: { outcome: "false-positive", cause: "false-positive", owner: "X" }, b: { outcome: "false-clean", certified: true, cause: "R-other" }, c: { outcome: "false-clean", certified: true, cause: "fam" }, d: { outcome: "false-clean", certified: true, cause: "R-F2-open" } } } };
    expect(ownerProblems({ rows, owners: { "g/a": "X", "g/b": "Y" } })).toEqual(["g/b: the row says no owner, owners says Y"]);
    expect(ownerProblems({ rows, owners: { "g/a": "X" } })).toEqual(["g/b: certified under R-other with no owner"]);
    expect(ownerProblems({ rows, owners: { "g/a": "X", "g/z": "X" } })).toEqual(["g/z: an owner for no ledger row", "g/b: certified under R-other with no owner"]);
    expect(ownerProblems(readOpenDefects())).toEqual([]);
  });
  it("lists no row of ADR 0866's contradiction: those are R5's, and fail the smoke", () => {
    for (const rows of Object.values(readOpenDefects().rows.grids)) for (const id of Object.keys(rows)) expect(id).not.toMatch(/-flw-guard-(?:early|probe-loop)-/);
  });
  it("a stop row stays held only while its listed uncertifiable stands, on every run (R27)", () => {
    const named = { id: "SC9012", blocker: "value-in-carrier", line: 5, column: 7 };
    const stop = { rows: { grids: { g: { "mx-a-1": { outcome: "false-clean", cause: "R-stop-named-U", blocker: namedKey(named) } } } } };
    expect(gridStanding(stop, { ...row, detail: { namedU: [named] } }, "false-clean", { grid: "g" }).kind).toBe("held");
    expect(gridStanding(stop, { ...row, detail: {} }, "false-clean", { grid: "g" }).kind).toBe("moved");
    expect(gridStanding(stop, { ...row, detail: { namedU: [{ ...named, column: 9 }] } }, "false-clean", { grid: "g" }).kind).toBe("moved");
  });
  it("a CAUGHT row stops only on a read rule's row at Chrome's exact position, and keeps needing it", () => {
    const caught = { code: "CAUGHT_REACTIVE_READ", detail: { namedU: [{ rule: "missing-owner", id: "SC4001", blocker: "x", line: 5, column: 7, atRead: true }, { rule: "strict-read-untracked", id: "SC1001", blocker: "y", line: 5, column: 3 }] } };
    expect(stopFinding(caught)).toBe(null);
    const at = { rule: "strict-read-untracked", id: "SC1001", blocker: "y", line: 5, column: 3, atRead: true };
    expect(stopFinding({ ...caught, detail: { namedU: [at] } })).toBe(at);
    const stop = { rows: { grids: { g: { "mx-a-1": { outcome: "false-clean", cause: "R-stop-named-U", blocker: namedKey(at), atRead: true } } } } };
    expect(gridStanding(stop, { ...row, detail: { namedU: [at] } }, "false-clean", { grid: "g" }).kind).toBe("held");
    expect(gridStanding(stop, { ...row, detail: { namedU: [{ ...at, atRead: undefined }] } }, "false-clean", { grid: "g" }).kind).toBe("moved");
  });
  it("takes only the measured variants into the stop group", () => {
    const false_clean = { outcome: "false-clean", code: "X", context: "body" };
    expect(inStopGroup({ ...false_clean, variant: "store-proto-literal" })).toBe(true);
    for (const variant of ["installed-get", "installed-set", "proto-create", "proto-receiver", "store-proto-setprototypeof"]) expect(inStopGroup({ ...false_clean, variant }), variant).toBe(false);
    expect(inStopGroup({ outcome: "false-clean", code: "CAUGHT_REACTIVE_READ", context: "evt-memo-fire-focus" })).toBe(true);
    expect(inStopGroup({ outcome: "false-clean", code: "CAUGHT_REACTIVE_READ", context: "handed-flush" })).toBe(false);
  });
  it("leaves only manifest moves the ledger does not hold", () => {
    expect(offLedger(list, "g", [{ id: "mx-a-1" }, { id: "mx-a-2" }])).toEqual([{ id: "mx-a-2" }]);
  });
  it("finds a family by outcome: a later entry of the same key is reachable", () => {
    expect(heldGridEntry(list, row, "misplaced").item).toBe("other");
    expect(heldGridEntry(list, row, "false-clean").item).toBe("fam");
  });
  it("the list keeps no two entries of one key and outcome, no `partial`, and no cause above its ratchet", () => {
    const open = readOpenDefects();
    const keys = open.grids.map((entry) => JSON.stringify([entry.idPrefix, entry.code, entry.variantPrefix, entry.contexts, entry.source, entry.outcome]));
    expect(new Set(keys).size).toBe(keys.length);
    expect(open.grids.filter((entry) => "partial" in entry)).toEqual([]);
    expect(ratchetDifferences(open)).toEqual([]);
  });
  it("the ledger is there and not empty, and every ratchet cause has rows", () => {
    const open = readOpenDefects();
    expect(Object.keys(open.rows.grids).length).toBeGreaterThan(0);
    expect(Object.keys(open.ratchet.causes).length).toBeGreaterThan(0);
    for (const count of Object.values(open.ratchet.causes)) expect(count).toBeGreaterThan(0);
  });
  it("a lost ledger throws", () => {
    expect(() => readOpenDefects(undefined, "/nonexistent/rows.json")).toThrow(/missing/);
  });
  it("an unattributed corpus site blocks the final gate by name, with an owner and a cause (Part 16)", () => {
    const row = { code: "SC9012", blocker: "b", app: "a", path: "a/x.ts", line: 3, start: 10, end: 12, owner: "C2", cause: "c" };
    expect(finalBlockers({ rows: { grids: {} }, corpusBlockers: [row] })).toEqual(["corpus/a/x.ts:3:10-12"]);
    for (const entry of readOpenDefects().corpusBlockers ?? []) for (const key of ["code", "blocker", "path", "line", "start", "end", "owner", "cause"]) expect(entry[key], `${entry.path}:${entry.line} ${key}`).toBeDefined();
  });
  it("stacked claims count exactly, in all and per pair, and do not block #173 (ADR 0887)", () => {
    const rows = { grids: { g: { a: { outcome: "stacked-claim", cause: "R-stacked", pairs: ["A|r"] }, b: { outcome: "stacked-claim", cause: "R-stacked", pairs: ["A|r", "B|s"] } } } };
    expect(ledgerCounts(rows)).toMatchObject({ stackedClaim: 2, stackedPairs: { "A|r": 2, "B|s": 1 } });
    const base = { certified: 0, certifiedF2: 0, falsePositive: 0, causes: { "R-stacked": 2 } };
    expect(ratchetDifferences({ rows, ratchet: { ...base, stackedClaim: 2, stackedPairs: { "A|r": 2, "B|s": 1 } } })).toEqual([]);
    expect(ratchetDifferences({ rows, ratchet: { ...base, stackedClaim: 2, stackedPairs: { "A|r": 2 } } })).toEqual([{ key: "stacked B|s", rows: 1, ratchet: null }]);
    expect(ratchetDifferences({ rows, ratchet: base }).map((row) => row.key)).toEqual(["stackedClaim", "stacked A|r", "stacked B|s"]);
    expect(finalBlockers({ rows })).toEqual([]);
    const unjudged = { grids: { g: { c: { outcome: "unjudged-claim", cause: "R-unjudged-unplaced", rules: ["r"] } } } };
    expect(ledgerCounts(unjudged)).toMatchObject({ unjudgedClaim: 1 });
    expect(ratchetDifferences({ rows: unjudged, ratchet: { certified: 0, certifiedF2: 0, falsePositive: 0, causes: { "R-unjudged-unplaced": 1 } } }).map((row) => row.key)).toEqual(["unjudgedClaim"]);
  });
  it("the ratchet is exact both ways, certified and false-positive rows on their own", () => {
    const rows = { grids: { g: { a: { outcome: "false-clean", certified: true, cause: "R" }, b: { outcome: "false-positive", cause: "R" } } } };
    expect(ratchetDifferences({ rows, ratchet: { certified: 1, certifiedF2: 0, falsePositive: 1, causes: { R: 2 } } })).toEqual([]);
    expect(ratchetDifferences({ rows, ratchet: { certified: 1, certifiedF2: 0, falsePositive: 1, causes: { R: 3 } } })).toEqual([{ key: "R", rows: 2, ratchet: 3 }]);
    expect(ratchetDifferences({ rows, ratchet: { certified: 0, certifiedF2: 0, falsePositive: 1, causes: { R: 2 } } })).toEqual([{ key: "certified", rows: 1, ratchet: 0 }]);
  });
});

describe("open-rows.mjs (R25)", () => {
  it("refuses a run with no report, and says what an update adds and removes", () => {
    expect(() => runSeed("/nonexistent")).toThrow(/did not finish/);
    expect(ledgerChange({ grids: { g: { a: {}, b: {} } } }, { grids: { g: { b: {}, c: {} } } })).toEqual({ added: ["g/c"], removed: ["g/a"], moved: [] });
    expect(ledgerChange({ grids: { g: { a: { cause: "R" } } } }, { grids: { g: { a: { cause: "S" } } } }).moved).toEqual([{ id: "g/a", from: "R", to: "S" }]);
  });
});
