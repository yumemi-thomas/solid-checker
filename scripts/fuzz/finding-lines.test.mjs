import { describe, expect, it } from "vitest";
import { holdsWholeFile, lastLine } from "./finding-lines.mjs";

// ADR 1152: a file-wide obligation holds every line of its file; any other finding holds the lines its report names.

describe("finding-lines", () => {
  const at = (extra) => ({ kind: "uncertifiable", subjectKind: "", primaryLocation: { line: 1, column: 1 }, ...extra });

  it("takes an uncertifiable result of subject `file` for one that holds the whole file", () => {
    expect(holdsWholeFile(at({ subjectKind: "file" }))).toBe(true);
    expect(lastLine(at({ subjectKind: "file" }))).toBe(Number.MAX_SAFE_INTEGER);
    // A violation of subject `file` (a proven compiler refusal) and a notice of subject `project` are no such obligation.
    expect(holdsWholeFile(at({ kind: "violation", subjectKind: "file" }))).toBe(false);
    expect(holdsWholeFile(at({ subjectKind: "project" }))).toBe(false);
    expect(holdsWholeFile(at({}))).toBe(false);
  });

  it("holds the end line a report names, else the start line", () => {
    expect(lastLine(at({ primaryLocation: { line: 8, column: 3, endLine: 12 } }))).toBe(12);
    expect(lastLine(at({ primaryLocation: { line: 8, column: 3 } }))).toBe(8);
  });
});
