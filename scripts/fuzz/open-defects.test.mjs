import { existsSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { gridStanding, heldGridEntry, readOpenDefects } from "./open-defects.mjs";
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
      expect(["false-positive", "false-clean", "misplaced", "no-run"]).toContain(entry.outcome);
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
  it("holds a grid row only for its own outcome, and calls a passing one fixed", () => {
    const entries = { grids: [{ idPrefix: "lg-", code: "X", variantPrefix: "noop-", outcome: "false-positive", item: "V-D1" }] };
    const row = { id: "lg-x", code: "X", variant: "noop-a", outcome: "false-positive" };
    expect(heldGridEntry(entries, { ...row, variant: "other" })).toBe(null);
    expect(gridStanding(entries, row, "false-positive").kind).toBe("held");
    expect(gridStanding(entries, { ...row, outcome: "uncertifiable" }, null).kind).toBe("fixed");
    expect(gridStanding(entries, row, "harness")).toBe(null);
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
