import { describe, expect, it } from "vitest";
import { gridRows } from "./cover.mjs";
import { buildRow, parseOptions, readIds, reduceReport, selected } from "./blocker-scan.mjs";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

// ADR 0954: the scan's pure parts. The checker is the grids' business; what is tested here is which programs a selection names, how a report
// is reduced to the line the join reads, and that a row is rebuilt as the grid built it.
const base = ["--checker-bin", "/bin/checker", "--typefacts", "/bin/typefacts", "--out", "/tmp/out.jsonl"];

describe("options", () => {
  it("need a checker, a producer and a fresh output", () => {
    expect(() => parseOptions([])).toThrow(/--checker-bin is required/);
    expect(() => parseOptions(["--checker-bin", "a", "--typefacts", "b"])).toThrow(/--out is required/);
  });
  it("refuse a grid, a fraction, a shard and a flag they do not know", () => {
    expect(() => parseOptions([...base, "--grids", "mx-nope"])).toThrow(/unknown grid mx-nope/);
    expect(() => parseOptions([...base, "--sample", "0"])).toThrow(/fraction/);
    expect(() => parseOptions([...base, "--sample", "1.5"])).toThrow(/fraction/);
    expect(() => parseOptions([...base, "--shard", "3/3"])).toThrow(/i < N/);
    expect(() => parseOptions([...base, "--bogus", "1"])).toThrow(/unknown or incomplete option/);
  });
  it("read lists and numbers", () => {
    const options = parseOptions([...base, "--grids", "mx-recall,lg-recall", "--codes", "A,B", "--sample", "0.25", "--shard", "1/4", "--concurrency", "3"]);
    expect(options.grids).toEqual(["mx-recall", "lg-recall"]);
    expect(options.codes).toEqual(["A", "B"]);
    expect([options.sample, options.shard, options.concurrency]).toEqual([0.25, [1, 4], 3]);
    expect(parseOptions(base).grids).toContain("mx-census");
  });
});

describe("selection", () => {
  const rows = gridRows("mx-recall", { codes: ["UNTRACKED_READ_AFTER_AWAIT"] });
  it("is a function of the id and the options, so two runs scan the same programs", () => {
    const options = parseOptions([...base, "--sample", "0.2", "--sample-seed", "7"]);
    const once = rows.filter((row) => selected(row, options)).map((row) => row.id);
    const again = rows.filter((row) => selected(row, options)).map((row) => row.id);
    expect(once).toEqual(again);
    expect(once.length).toBeGreaterThan(rows.length * 0.1);
    expect(once.length).toBeLessThan(rows.length * 0.3);
    const other = parseOptions([...base, "--sample", "0.2", "--sample-seed", "8"]);
    expect(rows.filter((row) => selected(row, other)).map((row) => row.id)).not.toEqual(once);
  });
  it("shards partition the rows", () => {
    const shards = [0, 1, 2].map((index) => rows.filter((row) => selected(row, parseOptions([...base, "--shard", `${index}/3`]))).map((row) => row.id));
    expect(shards.flat().sort()).toEqual(rows.map((row) => row.id).sort());
    expect(new Set(shards.flat()).size).toBe(rows.length);
  });
  it("narrows by context and by an id list", () => {
    const context = parseOptions([...base, "--contexts", "aw-helper-late"]);
    const chosen = rows.filter((row) => selected(row, context));
    expect(chosen.length).toBeGreaterThan(0);
    expect(chosen.every((row) => row.context === "aw-helper-late")).toBe(true);
    const dir = mkdtempSync(join(tmpdir(), "blocker-scan-"));
    try {
      writeFileSync(join(dir, "ids.json"), JSON.stringify({ ids: [rows[0].id, rows[1].id] }));
      writeFileSync(join(dir, "ids.txt"), `${rows[2].id}\n\n${rows[3].id}\n`);
      expect([...readIds(join(dir, "ids.json"))]).toEqual([rows[0].id, rows[1].id]);
      expect([...readIds(join(dir, "ids.txt"))]).toEqual([rows[2].id, rows[3].id]);
      expect(rows.filter((row) => selected(row, parseOptions(base), readIds(join(dir, "ids.json")))).length).toBe(2);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});

describe("a report reduced to a scan line", () => {
  const root = "/work/p1";
  const at = (file, line, endLine = line) => ({ path: `${root}/${file}`, line, endLine });
  const finding = (kind, file, line, blockedBy, endLine) => ({ kind, id: "SC9012", primaryLocation: at(file, line, endLine), ...(blockedBy ? { blockedBy } : {}) });
  const handed = { kind: "standard-member-impostor", scope: "program", formula: "handed-root-patch", fired: ["StandardRootHanded", "OpenMemberWrite(*)"] };
  const report = {
    findings: [
      finding("uncertifiable", "App.tsx", 6, handed),
      finding("uncertifiable", "App.tsx", 20, handed),
      finding("uncertifiable", "App.tsx", 7, { kind: "function-run-unplaced" }),
      finding("uncertifiable", "App.tsx", 31, undefined),
      finding("uncertifiable", "helpers.ts", 6, { kind: "provider-not-proven-above", missing: { fact: "provider-above", member: "Ctx" } }),
      finding("violation", "App.tsx", 6),
      finding("violation", "App.tsx", 40),
      { kind: "uncertifiable", id: "SC9012", primaryLocation: { path: "/elsewhere/App.tsx", line: 6, endLine: 6 }, blockedBy: handed },
    ],
  };
  const reduced = reduceReport(report, root, { line: 6, aux: [8] });
  it("counts every uncertifiable finding of the program's files by kind, at the site or away", () => {
    expect(reduced.total).toBe(5);
    expect(reduced.kinds).toEqual({
      "function-run-unplaced": [0, 1],
      none: [0, 1],
      "provider-not-proven-above": [0, 1],
      "standard-member-impostor": [1, 1],
    });
  });
  it("keeps the rows whose blocker names a formula or a missing fact, with the fired set sorted, and where they stand", () => {
    expect(reduced.formulas).toEqual([
      { rule: "SC9012", kind: "standard-member-impostor", formula: "handed-root-patch", fired: ["OpenMemberWrite(*)", "StandardRootHanded"], missing: null, line: 6, atSite: true },
      { rule: "SC9012", kind: "standard-member-impostor", formula: "handed-root-patch", fired: ["OpenMemberWrite(*)", "StandardRootHanded"], missing: null, line: 20, atSite: false },
      { rule: "SC9012", kind: "provider-not-proven-above", formula: null, fired: [], missing: { fact: "provider-above", member: "Ctx" }, line: 6, atSite: false },
    ]);
  });
  it("counts the violations in the site region, and a finding of another module is never at the site", () => {
    expect(reduced.violationsAtSite).toBe(1);
    // a multi-line finding that holds an aux line of the region is at the site
    const spanning = reduceReport({ findings: [finding("uncertifiable", "App.tsx", 7, handed, 9)] }, root, { line: 6, aux: [8] });
    expect(spanning.formulas[0].atSite).toBe(true);
  });
});

describe("a row rebuilt as the grid built it", () => {
  it("has the id the grid gave it, on both sides of the grids", () => {
    const misuse = gridRows("mx-recall", { codes: ["UNTRACKED_READ_AFTER_AWAIT"] }).find((row) => row.context === "aw-helper-late");
    const legal = gridRows("lg-recall", { codes: ["UNTRACKED_READ_AFTER_AWAIT"] })[0];
    expect(buildRow(misuse).id).toBe(misuse.id);
    expect(buildRow(legal).id).toBe(legal.id);
    expect(buildRow(misuse).files["App.tsx"]).toContain("createMemo");
  });
  it("refuses a row whose id is not the program's", () => {
    const row = gridRows("mx-recall", { codes: ["UNTRACKED_READ_AFTER_AWAIT"] })[0];
    expect(() => buildRow({ ...row, id: "mx-not-the-id" })).toThrow(/id mismatch/);
  });
});
