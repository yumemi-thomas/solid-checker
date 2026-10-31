import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { GRIDS } from "./focus.mjs";
import { generateLegal, legalCells, legalShapes } from "./legal.mjs";
import { cellList, combined, generateShape, gridShapes, mountsOption } from "./matrix.mjs";
import { sourcesOption } from "./misuse.mjs";
import { readRediscovery } from "./smoke.mjs";

const here = dirname(fileURLToPath(import.meta.url));
// A grid's ids for one code, as `make fuzz-focus CODES=<code>` generates them (focus.mjs GRIDS: the full run's arguments).
function gridIds(grid, code) {
  const [script, ...args] = GRIDS[grid];
  const value = (name) => { const at = args.indexOf(name); return at < 0 ? undefined : args[at + 1]; };
  const mounts = mountsOption(value("--mounts")), sources = sourcesOption(value("--sources")), combine = args.includes("--combine");
  if (script === "legal.mjs") return legalCells({ codes: [code] }).flatMap((cell) => combined(legalShapes(code, mounts, sources, cell.context), combine).map((shape) => generateLegal({ seed: 1 }, cell, { shape }).built.id));
  return cellList({ codes: [code] }).flatMap((cell) => combined(gridShapes(code, cell.context, mounts, sources), combine).map((shape, index) => generateShape({ seed: 1 }, cell, shape, index).built.id));
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
    for (const entry of registry.filter((e) => !e.outside)) {
      const generated = new Set(gridIds(entry.grid, entry.code));
      expect(entry.ids.filter((id) => !generated.has(id))).toEqual([]);
      const manifest = join(here, "smoke", `${entry.grid}.json`);
      if (existsSync(manifest)) {
        const held = new Set(JSON.parse(readFileSync(manifest, "utf8")).ids);
        expect(entry.ids.filter((id) => !held.has(id))).toEqual([]);
      }
    }
  }, 120000);
});
