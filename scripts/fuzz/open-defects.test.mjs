import { existsSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { gridStanding, heldGridEntry, readOpenDefects } from "./open-defects.mjs";
import { PATTERNS, comparePatterns, judgePattern, patternCases } from "./patterns.mjs";

const finding = (rule, kind) => ({ rule, kind, primaryLocation: { path: "App.tsx" } });

describe("the known open list", () => {
  const list = readOpenDefects();
  it("says how much is open, and names a queue item and a program for each", () => {
    const count = (outcome) => list.patterns.filter((entry) => entry.outcome === outcome).length;
    console.log(`known open: ${list.patterns.length} patterns (${count("false-positive")} false positive, ${count("false-clean")} false clean), ${list.grids.length} grid families`);
    for (const entry of list.patterns) {
      expect(entry.item).toBeTruthy();
      expect(["false-positive", "false-clean", "no-run"]).toContain(entry.outcome);
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
    expect(judgePattern({ codes: ["STRICT_READ_UNTRACKED"] }, [])).toMatchObject({ outcome: "false-clean", certified: true });
  });
  it("a violation Chrome did not report is a false positive", () => {
    expect(judgePattern({ codes: [] }, [finding("strict-read-untracked", "violation")]).outcome).toBe("false-positive");
  });
  it("an obligation answers a code, and a failed page is no verdict", () => {
    expect(judgePattern({ codes: ["STRICT_READ_UNTRACKED"] }, [finding("strict-read-untracked", "uncertifiable")]).outcome).toBe("fine");
    expect(judgePattern({ codes: [], failure: "x" }, []).outcome).toBe("no-run");
  });
});
