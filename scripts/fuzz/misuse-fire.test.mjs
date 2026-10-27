import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { buildCase, writeCase } from "./emit.mjs";
import { DEFAULT_FAST_INSTALL, runFast } from "./fast.mjs";
import { generateProgram } from "./generate.mjs";
import { cellList, cellSeed, generateCell, judge, siteOf } from "./matrix.mjs";

describe("the matrix's cells", () => {
  it("keeps a cell's programs where they are when other cells change", () => {
    expect(cellSeed(1, "A", "body", 0)).toBe(cellSeed(1, "A", "body", 0));
    expect(cellSeed(1, "A", "body", 0)).not.toBe(cellSeed(1, "A", "memo", 0));
    expect(cellSeed(1, "A", "body", 0)).not.toBe(cellSeed(2, "A", "body", 0));
  });
});

describe("every template fires its code in the fast runtime", () => {
  it("reports the code at the site (or, for a placeless code, anywhere) in every cell", async () => {
    const root = mkdtempSync(join(tmpdir(), "fuzz-misuse-"));
    try {
      const entries = [];
      for (const cell of cellList()) for (const index of [0]) entries.push({ cell, ...generateCell({ seed: 1 }, cell, index) });
      const cases = entries.map(({ built }) => ({ id: built.id, dir: writeCase(join(root, built.id), built), variant: "run" }));
      const records = await runFast(cases, { install: DEFAULT_FAST_INSTALL, workers: 4 });
      const missed = [];
      for (const { cell, built, program } of entries) {
        const verdict = judge({ code: cell.code, site: siteOf(built), typecheck: { ok: true }, checker: { ok: true, findings: [] }, runtime: records.get(built.id) });
        if (verdict.outcome === "not-fired" || verdict.outcome === "harness") missed.push(`${built.id} (${program.components[0].shape}): ${verdict.outcome} ${JSON.stringify(verdict.detail)}`);
      }
      expect(missed).toEqual([]);
    } finally { rmSync(root, { recursive: true, force: true }); }
  }, 300000);
});

describe("buildCase of a misuse program", () => {
  it("names the site's code and aux lines in sites.json", () => {
    const built = buildCase(generateProgram(5, { profile: "misuse", misusePlan: { code: "NO_OWNER_EFFECT", context: "handler", binding: "wrapper" } }));
    const sites = JSON.parse(built.files["sites.json"]).sites;
    expect(sites[0]).toMatchObject({ kind: "use", code: "NO_OWNER_EFFECT", bind: "wrapper" });
    expect(built.neutral).toBe(true);
  });
});
