import { describe, expect, it } from "vitest";
import { OWNER_PROFILES, generateProgram } from "./generate.mjs";
import { getAt } from "./model.mjs";
import { nodeAtLine, nodeLabel, nodesAtLine, printProgram } from "./print.mjs";

const profiles = [undefined, "closure-escape", "body-call", "default-param", "helper-closure", "in-place", "async", ...Object.keys(OWNER_PROFILES)];
const programs = profiles.flatMap((profile) => Array.from({ length: 25 }, (_, index) => generateProgram(index + 1, { profile })));

describe("the line table", () => {
  it("leaves the printed text without markers", () => {
    for (const program of programs) {
      const printed = printProgram(program);
      for (const text of [printed.text, printed.runText]) expect(text).not.toMatch(/[\u0001-\u0006]/);
    }
  });
  it("gives every site and every glue call the line the printer found for it, from the path alone", () => {
    for (const program of programs) {
      const { sites, glue, lineTable } = printProgram(program);
      for (const known of [...sites, ...glue]) {
        const row = lineTable.find((candidate) => JSON.stringify(candidate.path) === JSON.stringify(known.path) && candidate.line === known.line);
        expect(row, `${known.id} at line ${known.line}`).toBeDefined();
        expect(row.endLine).toBeGreaterThanOrEqual(row.line);
      }
    }
  });
  it("is the same for the clean text and the twin, and does not depend on instrumentation", () => {
    for (const program of programs.slice(0, 40)) expect(printProgram(program, { instrument: false }).lineTable).toEqual(printProgram(program).lineTable);
  });
  it("labels each path with the node the path names", () => {
    for (const program of programs.slice(0, 80)) {
      for (const row of printProgram(program).lineTable) expect(nodeLabel(getAt(program, row.path))).toBe(row.label);
    }
  });
  it("finds the innermost node covering a line", () => {
    const table = [
      { line: 3, endLine: 9, path: ["a"], label: "prim:memo" },
      { line: 4, endLine: 6, path: ["a", "b"], label: "fn:arrow" },
      { line: 5, endLine: 5, path: ["a", "b", "c"], label: "own:cleanup" },
    ];
    expect(nodeAtLine(table, 5).label).toBe("own:cleanup");
    expect(nodesAtLine(table, 5).map((row) => row.label)).toEqual(["own:cleanup", "fn:arrow", "prim:memo"]);
    expect(nodeAtLine(table, 8).label).toBe("prim:memo");
    expect(nodeAtLine(table, 12)).toBeNull();
  });
});
