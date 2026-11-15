import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { BLOCKER_KINDS, compareRuns, compareUnit, pairKind, programFiles, reportLines } from "./boundary-twin.mjs";
import { boundaryManifest, boundaryOverride, checkerProjectFiles, entryOverride, enteredExports, mainSourceFor, namedEntry } from "./checker.mjs";
import { parse } from "./focus.mjs";
import { buildCase } from "./emit.mjs";
import { generateProgram } from "./generate.mjs";

// ADR 0990: the boundary twin (`focus.mjs --boundary both`), the named entry (`--entry named`) and the comparison of the two runs.

const row = (id, outcome, detail = {}) => ({ id, outcome, detail });

describe("the boundary override", () => {
  it("is read from the environment, open or closed, and refuses another value", () => {
    expect(boundaryOverride({})).toBeUndefined();
    expect(boundaryOverride({ SOLID_CHECKER_FUZZ_BOUNDARY: "" })).toBeUndefined();
    expect(boundaryOverride({ SOLID_CHECKER_FUZZ_BOUNDARY: "open" })).toBe("open");
    expect(boundaryOverride({ SOLID_CHECKER_FUZZ_BOUNDARY: "closed" })).toBe("closed");
    expect(() => boundaryOverride({ SOLID_CHECKER_FUZZ_BOUNDARY: "ajar" })).toThrow(/open or closed/);
  });
  it("wins over the case's own `programBoundary`, which is the manifest's otherwise", () => {
    const own = JSON.stringify({ programBoundary: "open" });
    expect(JSON.parse(boundaryManifest(own, "x1", undefined))).toEqual({ name: "x1", main: "main.tsx" });
    expect(JSON.parse(boundaryManifest(own, "x1", "closed"))).toEqual({ name: "x1", private: true });
    expect(JSON.parse(boundaryManifest(undefined, "x1", "open"))).toEqual({ name: "x1", main: "main.tsx" });
    expect(boundaryManifest(undefined, "x1", undefined)).toBeUndefined();
  });
  it("is a file of the checker's project only when set", () => {
    const built = buildCase(generateProgram(7, { profile: "misuse", misusePlan: { code: "FLUSH_IN_ACTION", context: "action", binding: "direct", layer: "none", variant: null } }), { id: "x1" });
    expect(Object.keys(checkerProjectFiles(built))).not.toContain("package.json");
  });
  it("is an option of focus.mjs: open, closed or both, and none of a smoke run's", () => {
    const base = ["--codes", "FLUSH_IN_ACTION"];
    expect(parse([...base, "--boundary", "both"]).boundary).toBe("both");
    expect(parse([...base, "--boundary", "open", "--entry", "named"]).entry).toBe("named");
    expect(() => parse([...base, "--boundary", "ajar"])).toThrow(/open, closed or both/);
    expect(() => parse([...base, "--entry", "all"])).toThrow(/namespace or named/);
    expect(() => parse(["--smoke", "--boundary", "open"])).toThrow(/smoke/);
    expect(() => parse(["--smoke", "--entry", "named"])).toThrow(/smoke/);
  });
});

describe("the named entry", () => {
  it("is read from the environment", () => {
    expect(entryOverride({})).toBeUndefined();
    expect(entryOverride({ SOLID_CHECKER_FUZZ_ENTRY: "namespace" })).toBeUndefined();
    expect(entryOverride({ SOLID_CHECKER_FUZZ_ENTRY: "named" })).toBe("named");
    expect(() => entryOverride({ SOLID_CHECKER_FUZZ_ENTRY: "both" })).toThrow(/namespace or named/);
  });
  // The loop enters `default` and the capitalised names whose value is a function, in the namespace's (sorted) order.
  it("enters the exports the namespace loop enters, and leaves the others unentered", () => {
    const entered = (text) => enteredExports(text);
    expect(entered("export function C1() { return <p />; }\nexport function B2() { return <p />; }")).toEqual(["B2", "C1"]);
    // not capitalised, not a function
    expect(entered("export function C1() {}\nexport function hx1() {}\nexport const n = () => 1;\nexport const Count = 3;\nexport const List = [1];\nexport const Obj = {};")).toEqual(["C1"]);
    expect(entered("export const K1 = () => <i />;\nexport const L1 = function () { return 1; };\nexport class M1 {}\nexport async function* N1() {}")).toEqual(["K1", "L1", "M1", "N1"]);
    // a specifier gives the exported name; the local name does not count
    expect(entered("function local() {}\nconst Held = () => 1;\nexport { local as Shown, Held, local };")).toEqual(["Held", "Shown"]);
    expect(entered("export default function () { return 1; }\nexport function C1() {}")).toEqual(["C1", "default"]);
    expect(entered("const f = () => 1;\nexport default f;")).toEqual(["default"]);
    expect(entered("export type T = number;\nexport interface I {}\nexport function C1() {}")).toEqual(["C1"]);
    expect(entered("const a = () => 1;\nconst b = a;\nexport { b as B };")).toEqual(["B"]);
  });
  it("gives no entry for an export it cannot classify, which keeps the namespace one", () => {
    expect(enteredExports("export const C1 = make();")).toBeNull();
    expect(enteredExports("export * from \"./other\";")).toBeNull();
    expect(enteredExports("export { C1 } from \"./other\";")).toBeNull();
    expect(enteredExports("export const { C1 } = obj;")).toBeNull();
    expect(enteredExports("const x = make();\nexport { x as X };")).toBeNull();
    expect(namedEntry(undefined, "export const C1 = make();")).toBeNull();
  });
  it("renders each entered export by name in the oracle's order and carries the oracle's event driver over byte for byte", () => {
    const text = namedEntry(undefined, "export function C1() {}\nexport function B2() {}\nexport const hx = () => 1;");
    expect(text).toContain('import { B2, C1 } from "./App";');
    expect(text).not.toContain("import * as mod");
    expect(text).not.toContain("Object.entries");
    expect(text.indexOf("<B2 />")).toBeLessThan(text.indexOf("<C1 />"));
    expect(text).not.toContain("hx");
    const driver = (source) => source.slice(source.indexOf("// Drive every handler"));
    expect(driver(text)).toBe(driver(mainSourceFor(undefined)));
    expect(driver(text).length).toBeGreaterThan(500);
    // each render keeps the loop's failure handling
    expect(text).toContain('console.error("render failed", "B2", error);');
    expect(text).toContain("setTimeout(() => { throw error; });");
  });
  it("renders the mount names first, as the oracle's mount form does, then the other entered exports", () => {
    const oracle = JSON.stringify({ mount: ["K1", "C1"] });
    const text = namedEntry(oracle, "export function A1() {}\nexport function C1() {}\nexport function K1() {}");
    expect(text).toContain('import { C1, K1, A1 } from "./App";');
    expect(text.indexOf("<C1 />")).toBeLessThan(text.indexOf("<K1 />"));
    expect(text.indexOf("<K1 />")).toBeLessThan(text.indexOf("<A1 />"));
    const reference = mainSourceFor(oracle);
    expect(reference.indexOf("<C1 />")).toBeLessThan(reference.indexOf("<K1 />"));
  });
  it("imports a default export under a name of its own, and enters the same names the oracle's loop would for every kind of generated program", () => {
    expect(namedEntry(undefined, "export default function () { return 1; }")).toContain('import { default as DefaultExport } from "./App";');
    expect(namedEntry(undefined, "export default function () { return 1; }")).toContain('render(() => <DefaultExport />, host("default"));');
    const built = buildCase(generateProgram(7, { profile: "misuse", misusePlan: { code: "FLUSH_IN_ACTION", context: "action", binding: "direct", layer: "none", variant: null } }), { id: "x1" });
    expect(enteredExports(built.files["App.tsx"])).toEqual(["C1"]);
  });
});

describe("the comparison of a program under both boundaries", () => {
  it("names a closed run silent where the open one is not, an open one silent where the closed is a violation, and an open false positive: the three blockers", () => {
    expect(BLOCKER_KINDS).toEqual(["closed-silent", "open-silent", "open-false-positive"]);
    expect(pairKind(row("a", "false-clean"), row("a", "recall-gap"))).toBe("closed-silent");
    expect(pairKind(row("a", "false-clean"), row("a", "agree"))).toBe("closed-silent");
    expect(pairKind(row("a", "agree"), row("a", "false-clean"))).toBe("open-silent");
    expect(pairKind(row("a", "clean"), row("a", "false-positive"))).toBe("open-false-positive");
    expect(pairKind(row("a", "uncertifiable"), row("a", "false-positive"))).toBe("open-false-positive");
  });
  it("counts a violation the open world hands off, and one it gains, as costs, and calls no difference none", () => {
    expect(pairKind(row("a", "agree"), row("a", "recall-gap"))).toBe("open-lost-violation");
    expect(pairKind(row("a", "agree"), row("a", "uncertifiable"))).toBe("open-lost-violation");
    expect(pairKind(row("a", "recall-gap"), row("a", "agree"))).toBe("open-gained");
    expect(pairKind(row("a", "uncertifiable"), row("a", "clean"))).toBe("open-moved");
    expect(pairKind(row("a", "agree"), row("a", "agree"))).toBeNull();
    expect(pairKind(row("a", "clean"), row("a", "clean"))).toBeNull();
  });
  it("reads a second code on the site's lines missed in a certified file as a silence too", () => {
    const secondary = (detail) => row("a", "agree", { secondary: detail });
    expect(pairKind(secondary({ X: "false-clean-certified" }), secondary({}))).toBe("closed-silent");
    expect(pairKind(secondary({}), secondary({ X: "false-clean-certified" }))).toBe("open-silent");
  });
  it("counts the pairs of a unit, the programs the open run lacks, and lists each difference", () => {
    const result = compareUnit([row("a", "agree"), row("b", "recall-gap"), row("c", "false-clean"), row("d", "agree")], [row("a", "agree"), row("b", "recall-gap"), row("c", "recall-gap")]);
    expect(result.programs).toBe(4);
    expect(result.missing).toBe(1);
    expect(result.transitions).toEqual({ "agree->agree": 1, "recall-gap->recall-gap": 1, "false-clean->recall-gap": 1 });
    expect(result.kinds).toEqual({ "closed-silent": 1 });
    expect(result.pairs.map((pair) => pair.id)).toEqual(["c"]);
  });
});

describe("the comparison of two run directories", () => {
  const write = (root, unit, rows) => {
    mkdirSync(join(root, unit), { recursive: true });
    writeFileSync(join(root, unit, "programs.jsonl"), rows.map((r) => JSON.stringify(r)).join("\n") + "\n");
  };
  it("reads each unit's rows from both, ignores case directories, and reports blockers, costs and a unit one run lacks", () => {
    const dir = mkdtempSync(join(tmpdir(), "boundary-twin-"));
    try {
      write(join(dir, "closed"), "mx-recall/A", [row("a", "agree"), row("b", "false-clean")]);
      write(join(dir, "open"), "mx-recall/A", [row("a", "recall-gap"), row("b", "recall-gap")]);
      write(join(dir, "closed"), "lg-recall/A", [row("c", "clean")]);
      write(join(dir, "open"), "lg-recall/A", [row("c", "clean")]);
      write(join(dir, "closed"), "mx-recall/B", [row("d", "agree")]);
      mkdirSync(join(dir, "closed", "mx-recall/A", "cases", "x"), { recursive: true });
      writeFileSync(join(dir, "closed", "mx-recall/A", "cases", "x", "programs.jsonl"), "not json\n");
      expect(programFiles(join(dir, "closed")).sort()).toEqual(["lg-recall/A/programs.jsonl", "mx-recall/A/programs.jsonl", "mx-recall/B/programs.jsonl"]);
      const report = compareRuns(join(dir, "closed"), join(dir, "open"));
      expect(report.blockers.map((b) => `${b.kind}:${b.id}`)).toEqual(["closed-silent:b"]);
      expect(report.costs.map((c) => `${c.kind}:${c.id}`)).toEqual(["open-lost-violation:a"]);
      expect(report.problems).toEqual(["mx-recall/B/programs.jsonl: the open run wrote no rows for this unit"]);
      const lines = reportLines(report).join("\n");
      expect(lines).toContain("BLOCKER closed-silent mx-recall/A b");
      expect(lines).toContain("named costs: 1");
      expect(lines).toContain("PROBLEM mx-recall/B/programs.jsonl");
    } finally { rmSync(dir, { recursive: true, force: true }); }
  });
});
