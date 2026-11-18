// A grid's program ids without generating a program (an id is its cell and shape: matrix.mjs shapeId, legal.mjs legalId). The rediscovery
// test checks its entries against them; focus.mjs counts the programs a run will judge with them before it starts.
import { GRIDS } from "./focus-grids.mjs";
import { legalCells, legalId, legalShapes, pairLegalCells, recallLegalShapes } from "./legal.mjs";
import { cellList, censusCells, censusShapes, combined, gridShapes, mountsOption, pairShapes, recallShapes, shapeId } from "./matrix.mjs";
import { SPEC, sourcesOption } from "./misuse.mjs";

/** A grid's ids for one code, as `make fuzz-focus CODES=<code>` generates them (GRIDS: the full run's arguments); `full` is `--recall-full`. */
export function gridIds(grid, code, { full = false } = {}) {
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

/** How many programs a grid holds for these codes (`undefined`: every code). */
export function gridCount(grid, codes, { full = false } = {}) {
  return (codes ?? Object.keys(SPEC)).reduce((sum, code) => sum + gridIds(grid, code, { full }).length, 0);
}
