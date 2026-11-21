import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { classify, codeOf, contextTexts, difference, escapeProject, findingRows, minimize, moveTotals, moves, readTrace, reportKeys, siteTable, summarize } from "./escape-sweep.mjs";

// ADR 1152: the escape sweep's own logic (the sweep runs the checker; these hold what it does with the answers).

const report = (findings) => ({ findings: findings.map(([path, line, id, kind, blocker, rule]) => ({ id, kind, rule: rule ?? "rule", analysisContext: blocker, primaryLocation: { path, line } })) });

describe("escape-sweep: what is compared", () => {
  it("keys a finding by file, line, code, kind and blocker, with no column and no message", () => {
    const keys = reportKeys(report([["/p/App.tsx", 7, "SC4001", "violation", "", "missing-owner"], ["/p/App.tsx", 9, "SC1001", "uncertifiable", "callee-callback-timing:Probe"]]), "/p");
    expect(keys).toEqual(["App.tsx:7|SC4001|violation|missing-owner", "App.tsx:9|SC1001|uncertifiable|callee-callback-timing"].sort());
    expect(contextTexts(report([["/p/App.tsx", 9, "SC1001", "uncertifiable", "function-run-unplaced:Probe"]]))).toEqual(["function-run-unplaced:Probe"]);
  });

  it("takes the difference as multisets and classes what each side has alone", () => {
    const a = ["A|SC4001|violation|x", "A|SC4001|violation|x", "B|SC6002|uncertifiable|y"];
    const b = ["A|SC4001|violation|x", "C|SC4001|uncertifiable|z", "D|SC2001|violation|w"];
    const delta = difference(a, b);
    expect(delta.lost).toEqual(["A|SC4001|violation|x", "B|SC6002|uncertifiable|y"]);
    expect(delta.gained).toEqual(["C|SC4001|uncertifiable|z", "D|SC2001|violation|w"]);
    expect(classify(delta)).toEqual(["gained-obligation", "gained-violation", "lost-obligation", "lost-violation"]);
    expect(difference(a, a)).toEqual({ lost: [], gained: [] });
    expect(codeOf("A|SC4001|violation|x")).toBe("SC4001");
  });
});

describe("escape-sweep: attribution", () => {
  it("finds the smallest set of names whose escape alone reproduces a divergence", async () => {
    const names = ["a", "b", "c", "d", "e", "f", "g", "h"];
    // Two names are needed together.
    const together = await minimize(names, async (subset) => subset.includes("c") && subset.includes("f"));
    expect(together.names.sort()).toEqual(["c", "f"]);
    // One name is enough, wherever it is.
    for (const one of names) {
      const alone = await minimize(names, async (subset) => subset.includes(one));
      expect(alone.names, one).toEqual([one]);
    }
    // Asked about few subsets, and never more than the limit.
    expect(together.asked).toBeLessThan(40);
    const limited = await minimize(names, async () => true, { limit: 3 });
    expect(limited.asked).toBeLessThanOrEqual(3);
  });

  it("tallies programs, classes, codes, minimal sets, names and the sites that read an escaped name", () => {
    const rows = [
      { id: "p1", status: "diverges", classes: ["lost-violation"], lost: ["A|SC4001|violation|x"], gained: ["A|SC4001|uncertifiable|y"], minimal: ["Misuse"], sites: [{ site: "rust/a.rs:10", text: "\\u004Disuse" }] },
      { id: "p2", status: "same", sites: [{ site: "rust/a.rs:10", text: "\\u0041pp" }, { site: "rust/b.rs:5", text: "ev\\u0061l" }] },
      { id: "p3", status: "diverges", classes: ["gained-obligation"], lost: [], gained: ["B|SC9012|uncertifiable|z"], minimal: ["Misuse"], sites: [] },
    ];
    const summary = summarize(rows);
    expect(summary.byStatus).toEqual([["diverges", 2], ["same", 1]]);
    expect(summary.byClass.map(([name]) => name).sort()).toEqual(["gained-obligation", "lost-violation"]);
    expect(summary.byMinimalSet).toEqual([["Misuse", 2]]);
    expect(summary.byName).toEqual([{ name: "Misuse", programs: 2, examples: ["p1", "p3"] }]);
    expect(summary.bySite).toEqual([
      { site: "rust/a.rs:10", programs: 2, diverging: 1, texts: ["\\u004Disuse", "\\u0041pp"], examples: ["p1", "p2"] },
      { site: "rust/b.rs:5", programs: 1, diverging: 0, texts: ["ev\\u0061l"], examples: ["p2"] },
    ]);
    expect(siteTable(rows)).toEqual(summary.bySite);
  });

  it("reads a trace file as its distinct reads", () => {
    const dir = mkdtempSync(join(tmpdir(), "escape-trace-"));
    try {
      writeFileSync(join(dir, "t.tsv"), "rust/a.rs:1\t/p/App.tsx\tev\\u0061l\nrust/a.rs:1\t/p/App.tsx\tev\\u0061l\nrust/b.rs:2\t/p/App.tsx\t\"x\"\n");
      expect(readTrace(join(dir, "t.tsv"))).toEqual([{ site: "rust/a.rs:1", text: "ev\\u0061l" }, { site: "rust/b.rs:2", text: '"x"' }]);
      expect(readTrace(join(dir, "missing.tsv"))).toEqual([]);
    } finally { rmSync(dir, { recursive: true, force: true }); }
  });
});

describe("escape-sweep: the project's files", () => {
  const project = () => {
    const root = mkdtempSync(join(tmpdir(), "escape-project-"));
    mkdirSync(join(root, "node_modules", "dep"), { recursive: true });
    mkdirSync(join(root, "node_modules", "solid-js"), { recursive: true });
    writeFileSync(join(root, "App.tsx"), "export function App() { return <p>{String(window.location)}</p>; }\n");
    writeFileSync(join(root, "node_modules", "dep", "index.js"), "export function run() { return document.title; }\n");
    writeFileSync(join(root, "node_modules", "solid-js", "index.js"), "export const createSignal = () => 1;\n");
    writeFileSync(join(root, "data.json"), '{ "a": 1 }\n');
    writeFileSync(join(root, "broken.ts"), "const = ;\n");
    return root;
  };

  it("escapes the project's sources, not a dependency's unless asked, never Solid's runtime, JSON or a file that does not parse", () => {
    const root = project();
    try {
      const plain = escapeProject(root);
      expect(plain.changed).toBe(1);
      expect(plain.unparsed).toEqual(["broken.ts"]);
      expect(readFileSync(join(root, "App.tsx"), "utf8")).not.toContain("window");
      expect(readFileSync(join(root, "node_modules", "dep", "index.js"), "utf8")).toContain("document.title");
      expect(readFileSync(join(root, "data.json"), "utf8")).toContain('"a"');
      const deps = escapeProject(root, { deps: true });
      expect(deps.changed).toBeGreaterThan(0);
      expect(readFileSync(join(root, "node_modules", "dep", "index.js"), "utf8")).not.toContain("document");
      expect(readFileSync(join(root, "node_modules", "solid-js", "index.js"), "utf8")).toContain("createSignal");
    } finally { rmSync(root, { recursive: true, force: true }); }
  });

  it("escapes only the names asked", () => {
    const root = project();
    try {
      const some = escapeProject(root, { names: ["window"] });
      expect([...some.names.keys()]).toEqual(["window"]);
      const text = readFileSync(join(root, "App.tsx"), "utf8");
      expect(text).not.toContain("window");
      expect(text).toContain("location");
      expect(text).toContain("String");
    } finally { rmSync(root, { recursive: true, force: true }); }
  });
});

describe("escape-sweep: how the sites move", () => {
  const rows = (list) => list.map(([file, line, id, kind, endLine]) => ({ file, line, endLine: endLine ?? line, id, kind }));

  it("reads a report as sites with the range each finding holds, violations and obligations only", () => {
    const found = findingRows({ findings: [
      { id: "SC2001", kind: "violation", primaryLocation: { path: "/p/App.tsx", line: 4, endLine: 6 } },
      { id: "SC9012", kind: "uncertifiable", primaryLocation: { path: "/p/App.tsx", line: 1, endLine: 20 } },
      { id: "SC0000", kind: "note", primaryLocation: { path: "/p/App.tsx", line: 2 } },
      { id: "SC8001", rule: "prefer-for", kind: "violation", primaryLocation: { path: "/p/App.tsx", line: 3 } },
    ] }, "/p");
    expect(found).toEqual([{ file: "App.tsx", line: 4, endLine: 6, id: "SC2001", kind: "V" }, { file: "App.tsx", line: 1, endLine: 20, id: "SC9012", kind: "U" }]);
  });

  it("holds every line of a file under its file-wide notice (an uncertifiable result of subject `file`), and no other line of another file", () => {
    const found = findingRows({ findings: [
      { id: "SC9012", kind: "uncertifiable", subjectKind: "file", primaryLocation: { path: "/p/App.tsx", line: 1, column: 1 } },
      { id: "SC2001", kind: "violation", primaryLocation: { path: "/p/App.tsx", line: 9, column: 3 } },
    ] }, "/p");
    expect(found[0].endLine).toBe(Number.MAX_SAFE_INTEGER);
    expect(found[1].endLine).toBe(9);
    const { counts, bad } = moves(rows([["App.tsx", 40, "SC2001", "V"], ["other.ts", 3, "SC2001", "V"]]), found.slice(0, 1));
    expect(counts).toMatchObject({ "V->U": 1, "V->clean": 1 });
    expect(bad).toEqual({ "V->clean": ["other.ts:3|SC2001"] });
  });

  it("calls a violation kept, obligated (a notice that spans the file included) or lost silently", () => {
    const original = rows([["a.tsx", 4, "SC2001", "V"], ["a.tsx", 9, "SC2001", "V"], ["b.tsx", 3, "SC2001", "V"], ["a.tsx", 12, "SC1001", "U"], ["a.tsx", 30, "SC1001", "U"]]);
    const escaped = rows([["a.tsx", 4, "SC2001", "V"], ["a.tsx", 1, "SC9012", "U", 20], ["a.tsx", 12, "SC1001", "U"]]);
    const { counts, bad } = moves(original, escaped);
    // a.tsx:4 is kept; a.tsx:9 and a.tsx:12 are held by the notice and by itself; b.tsx:3 is held by nothing; a.tsx:30 is outside the notice.
    expect(counts).toMatchObject({ "V->V": 1, "V->U": 1, "V->clean": 1, "U->U": 1, "U->clean": 1, "U->V": 0, "none->V": 0 });
    expect(bad).toEqual({ "V->clean": ["b.tsx:3|SC2001"] });
  });

  it("calls a violation of the escaped program that no violation of its code held a gained one, from an obligation or from nothing", () => {
    const original = rows([["a.tsx", 5, "SC1001", "U"], ["a.tsx", 8, "SC2001", "V"]]);
    const escaped = rows([["a.tsx", 5, "SC1001", "V"], ["a.tsx", 8, "SC1001", "V"], ["a.tsx", 40, "SC4001", "V"]]);
    const { counts, bad } = moves(original, escaped);
    // a.tsx:5 turned from an obligation into a violation; a.tsx:8 holds a violation of another code (the original's SC2001 is gone from the escaped program); a.tsx:40 is new.
    expect(counts).toMatchObject({ "U->V": 2, "none->V": 1, "V->U": 1, "V->clean": 0 });
    expect(Object.keys(bad).sort()).toEqual(["U->V", "none->V"]);
  });

  it("sums the moves of the programs and names the programs of the classes that must be empty", () => {
    const totals = moveTotals([
      { id: "p1", moves: { counts: { "V->V": 1, "V->clean": 1 }, bad: { "V->clean": ["a.tsx:3|SC2001"] } } },
      { id: "p2", moves: { counts: { "V->V": 2 }, bad: {} } },
      { id: "p3", status: "nothing-escaped" },
    ]);
    expect(totals).toEqual({ totals: { "V->V": 3, "V->clean": 1 }, bad: { "V->clean": [{ id: "p1", examples: ["a.tsx:3|SC2001"] }] } });
  });
});
