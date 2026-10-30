import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { GRIDS, readGrid } from "./focus.mjs";

describe("readGrid", () => {
  it("tallies the outcomes, flags what needs a look, and keeps only behaviour differences from the oracle", () => {
    const out = mkdtempSync(join(tmpdir(), "fuzz-focus-"));
    mkdirSync(out, { recursive: true });
    const rows = [{ id: "a", outcome: "agree" }, { id: "b", outcome: "false-clean", detail: { line: 3 } }, { id: "c", outcome: "recall-gap" }, { id: "d", outcome: "false-positive" }];
    writeFileSync(join(out, "programs.jsonl"), rows.map((row) => JSON.stringify(row)).join("\n"));
    writeFileSync(join(out, "runtime-diff.json"), JSON.stringify({
      summary: { compared: 3, behaviour: 1, attributionOnly: 1, inconclusive: 0 },
      differing: [{ id: "x", diffs: [{ kind: "moved", detail: "C happy@1 chrome@2" }] }, { id: "y", diffs: [{ kind: "chrome-only", detail: "C@4" }] }],
    }));
    const grid = readGrid(out);
    expect(grid.outcomes).toEqual({ agree: 1, "false-clean": 1, "recall-gap": 1, "false-positive": 1 });
    expect(grid.flagged.map((row) => row.id)).toEqual(["b", "d"]);
    expect(grid.behaviour.map((result) => result.id)).toEqual(["y"]);
    expect(readGrid(join(out, "missing"))).toEqual({ outcomes: {}, flagged: [], oracle: null, behaviour: [] });
  });

  it("runs the six grids of the full run", () => {
    expect(Object.keys(GRIDS)).toEqual(["mx-base", "mx-src", "mx-combo", "lg-base", "lg-src", "lg-combo"]);
  });
});
