import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { requiredIds, wholeGrid } from "./cover.mjs";
import { GRIDS } from "./focus.mjs";
import { legalCells, legalId, legalShapes, pairLegalCells, recallLegalShapes } from "./legal.mjs";
import { cellList, censusCells, censusShapes, combined, gridShapes, mountsOption, pairShapes, recallShapes, shapeId } from "./matrix.mjs";
import { sourcesOption } from "./misuse.mjs";
import { readRediscovery } from "./smoke.mjs";

const here = dirname(fileURLToPath(import.meta.url));
// A grid's ids for one code, as `make fuzz-focus CODES=<code>` generates them (focus.mjs GRIDS: the full run's arguments). An id is
// its cell and shape (matrix.mjs shapeId, legal.mjs legalId: the ids generateShape/generateLegal give), so no program is generated.
function gridIds(grid, code, { full = false } = {}) {
  const [script, ...args] = GRIDS[grid];
  const value = (name) => { const at = args.indexOf(name); return at < 0 ? undefined : args[at + 1]; };
  const mounts = mountsOption(value("--mounts")), sources = sourcesOption(value("--sources")), combine = args.includes("--combine");
  if (args.includes("--census")) return censusCells({ codes: [code] }).flatMap((cell) => censusShapes(code).map((shape) => shapeId(cell, shape)));
  // The pair grids (`--pairs`): their cells and pair shapes (matrix.mjs pairShapes).
  if (args.includes("--pairs")) {
    if (script === "legal.mjs") return pairLegalCells({ codes: [code] }).flatMap((cell) => pairShapes(code, cell.context, { legal: true }).map((shape) => legalId(cell, { shape })));
    return cellList({ codes: [code] }).flatMap((cell) => pairShapes(code, cell.context).map((shape) => shapeId(cell, shape)));
  }
  // The recall grids (`--recall`): their cells and the shapes the recall axes add (matrix.mjs recallAxes).
  if (args.includes("--recall")) {
    if (script === "legal.mjs") return legalCells({ codes: [code], recall: true }).flatMap((cell) => recallLegalShapes(code, cell.context, { lean: !full }).map((shape) => legalId(cell, { shape })));
    return cellList({ codes: [code], recall: true }).flatMap((cell) => recallShapes(code, cell.context, { lean: !full }).map((shape) => shapeId(cell, shape)));
  }
  if (script === "legal.mjs") return legalCells({ codes: [code] }).flatMap((cell) => combined(legalShapes(code, mounts, sources, cell.context), combine).map((shape) => legalId(cell, { shape })));
  return cellList({ codes: [code] }).flatMap((cell) => combined(gridShapes(code, cell.context, mounts, sources), combine).map((shape) => shapeId(cell, shape)));
}

// The rediscovery rule (AGENTS.md): every defect found any way is a shape the grids generate, or says why it cannot be one.
describe("rediscovery.json", () => {
  const registry = readRediscovery();
  it("names, for each defect, where it was found and the grid programs that rediscover it, or why there are none", () => {
    for (const entry of registry) {
      expect(entry.finding && entry.found).toBeTruthy();
      if (entry.outside) continue;
      expect(GRIDS[entry.grid]).toBeDefined();
      expect(entry.code && entry.ids?.length).toBeTruthy();
    }
  });

  it("lists programs the grids still generate, and the smoke set holds them", () => {
    // One id list per (grid, code): several entries share one (five name mx-recall STRICT_READ_UNTRACKED).
    const built = new Map();
    const idsOf = (grid, code, full = false) => {
      const key = `${grid}\0${code}\0${full}`;
      if (!built.has(key)) built.set(key, new Set(gridIds(grid, code, { full })));
      return built.get(key);
    };
    const required = new Set(requiredIds());
    for (const entry of registry.filter((e) => !e.outside)) {
      const generated = idsOf(entry.grid, entry.code);
      expect(entry.ids.filter((id) => !generated.has(id))).toEqual([]);
      // `fullIds`: programs only a `--recall-full` grid generates (a value `LEAN_DEMOTED` leaves out). Pinned beside `ids`, never in
      // place of them: a recall grid is lean by default, so `ids` are what `make fuzz-focus` rediscovers.
      if (entry.fullIds) {
        expect(entry.grid.endsWith("-recall"), `${entry.code}: fullIds belong to a recall grid`).toBe(true);
        const full = idsOf(entry.grid, entry.code, true);
        expect(entry.fullIds.filter((id) => !full.has(id))).toEqual([]);
        expect(entry.fullIds.filter((id) => generated.has(id)), "a lean grid generates it: it belongs in ids").toEqual([]);
      }
      const manifest = join(here, "smoke", `${entry.grid}.json`);
      if (existsSync(manifest)) {
        const held = new Set(JSON.parse(readFileSync(manifest, "utf8")).ids);
        expect(entry.ids.filter((id) => !held.has(id))).toEqual([]);
      }
      // ADR 0615: a grid with no manifest had these skipped (60 of 64 rows); every sample now holds them, the pull request's census sample too (ADR 0916).
      if (!wholeGrid(entry.grid, "all")) expect(entry.ids.filter((id) => !required.has(id))).toEqual([]);
    }
    // Generating the programs for their ids took 200 s on CI and timed out at 300 s (PR #162); the ids alone take seconds.
  }, 60000);
});
