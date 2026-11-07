// The pairwise samples (ADR 0615): a covering sample of every grid's programs, computed from the grids' shapes, so it is a function of the code
// alone: no run to read, no manifest to refresh, and a value a table adds is in the next sample. `--cover all`: every pair of axis values, across
// codes, the pull-request tier. `--cover per-code`: every pair of axis values within each code, the tier before a merge. A row is one program; a
// feature is a pair of its axis values; the sample is a greedy cover of the features, ties broken by a hash of the id under `seed`, so one seed gives one
// sample and only the nightly lane rotates it.
//
// The misuse and the legal grids are one population: `side` is an axis, so a value's pairs are covered on each side it has. A `required` program (the
// census's are their own grid; the ledger's, and the behaviour manifests') comes first and its features are not covered twice.
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { GRIDS } from "./focus-grids.mjs";
import { GENERATOR_VERSION } from "./generate.mjs";
import { legalCells, legalId, legalShapes, pairLegalCells, recallLegalShapes } from "./legal.mjs";
import { cellList, censusCells, censusShapes, combined, gridShapes, mountsOption, pairShapes, recallShapes, shapeId } from "./matrix.mjs";
import { sourcesOption } from "./misuse.mjs";
import { OPEN_ROWS } from "./open-defects.mjs";
import { fnv } from "./rng.mjs";
import { readRediscovery } from "./smoke.mjs";

/** The axes a pair is made of. */
export const AXES = ["side", "code", "context", "variant", "binding", "layer", "mount", "source", "callback", "combo"];
/** The grids a sample draws from. The census runs whole beside it. */
export const SAMPLED_GRIDS = Object.keys(GRIDS).filter((grid) => grid !== "mx-census");
/** The pull request's seed. It changes with the code or never: a failure belongs to the pull request that shows it. */
export const FIXED_SEED = 1;
const LEAN = { lean: true };

/** The (cell, shape) rows a grid generates, with their ids, as `make fuzz-focus` generates them: no program is built. */
export function gridRows(grid, { codes } = {}) {
  const [script, ...args] = GRIDS[grid];
  const value = (name) => { const at = args.indexOf(name); return at < 0 ? undefined : args[at + 1]; };
  const legal = script === "legal.mjs";
  const mounts = mountsOption(value("--mounts")), sources = sourcesOption(value("--sources")), combine = args.includes("--combine");
  const pairs = args.includes("--pairs"), recall = args.includes("--recall");
  const rows = [];
  const push = (cell, shape) => rows.push({ grid, id: legal ? legalId(cell, { shape }) : shapeId(cell, shape), side: legal ? "legal" : "misuse", code: cell.code, context: cell.context, ...shape });
  const cells = legal ? (pairs ? pairLegalCells({ codes }) : legalCells({ codes, recall })) : cellList({ codes, recall });
  for (const cell of cells) {
    const shapes = pairs ? pairShapes(cell.code, cell.context, { legal }) : recall ? (legal ? recallLegalShapes : recallShapes)(cell.code, cell.context, LEAN)
      : combined(legal ? legalShapes(cell.code, mounts, sources, cell.context) : gridShapes(cell.code, cell.context, mounts, sources), combine);
    for (const shape of shapes) push(cell, shape);
  }
  return rows;
}

/** A census grid's ids, for what asks of them: it runs whole. */
export const censusIds = ({ codes } = {}) => censusCells({ codes }).flatMap((cell) => censusShapes(cell.code).map((shape) => shapeId(cell, shape)));

const valueOf = (row, axis) => String(row[axis] ?? "-");
/** A row's features: every pair of its axis values; within its code with `perCode`, whose pairs leave `code` out and carry it. */
function eachFeature(row, perCode, emit) {
  const axes = perCode ? AXES.filter((axis) => axis !== "code") : AXES;
  const prefix = perCode ? `${row.code}\0` : "";
  const values = axes.map((axis) => valueOf(row, axis));
  for (let i = 0; i < axes.length; i += 1) for (let j = i + 1; j < axes.length; j += 1) emit(`${prefix}${i}.${values[i]}\0${j}.${values[j]}`);
}

/**
 * The greedy cover of `rows`: the ids, `required` first. Lazy: a row's gain only falls, so the heap's top is recounted and put back when it fell; a
 * top that still leads is the best row. Rows are held as a flat array of feature numbers, the 1.3 million programs' pairs being too many for sets.
 */
export function cover(rows, { perCode = false, seed = FIXED_SEED, required = [] } = {}) {
  const features = new Map();
  const starts = new Uint32Array(rows.length + 1);
  let flat = new Uint32Array(rows.length * 16 + 64);
  let used = 0;
  rows.forEach((row, index) => {
    eachFeature(row, perCode, (feature) => {
      let id = features.get(feature);
      if (id === undefined) { id = features.size; features.set(feature, id); }
      if (used === flat.length) { const grown = new Uint32Array(flat.length * 2); grown.set(flat); flat = grown; }
      flat[used] = id; used += 1;
    });
    starts[index + 1] = used;
  });
  const seen = new Uint8Array(features.size);
  const gainOf = (index) => { let gain = 0; for (let at = starts[index]; at < starts[index + 1]; at += 1) gain += seen[flat[at]] ^ 1; return gain; };
  const take = (index) => { for (let at = starts[index]; at < starts[index + 1]; at += 1) seen[flat[at]] = 1; };
  const wanted = new Set(required);
  const chosen = [];
  rows.forEach((row, index) => { if (wanted.has(row.id)) { take(index); chosen.push(row.id); } });
  const rank = new Uint32Array(rows.length);
  for (let index = 0; index < rows.length; index += 1) rank[index] = fnv(`${seed}:${rows[index].id}`);
  const gains = new Uint16Array(rows.length);
  const heap = [];
  const before = (a, b) => gains[a] > gains[b] || (gains[a] === gains[b] && (rank[a] < rank[b] || (rank[a] === rank[b] && a < b)));
  const siftUp = (at) => { const item = heap[at]; while (at > 0) { const up = (at - 1) >> 1; if (!before(item, heap[up])) break; heap[at] = heap[up]; at = up; } heap[at] = item; };
  const siftDown = (at) => {
    const item = heap[at];
    for (;;) {
      let child = at * 2 + 1;
      if (child >= heap.length) break;
      if (child + 1 < heap.length && before(heap[child + 1], heap[child])) child += 1;
      if (!before(heap[child], item)) break;
      heap[at] = heap[child]; at = child;
    }
    heap[at] = item;
  };
  for (let index = 0; index < rows.length; index += 1) { gains[index] = gainOf(index); if (gains[index] > 0) { heap.push(index); siftUp(heap.length - 1); } }
  while (heap.length) {
    const top = heap[0];
    const gain = gainOf(top);
    if (gain === 0) { const last = heap.pop(); if (heap.length) { heap[0] = last; siftDown(0); } continue; }
    if (gain < gains[top]) { gains[top] = gain; siftDown(0); continue; }
    take(top); chosen.push(rows[top].id);
    const last = heap.pop();
    if (heap.length) { heap[0] = last; siftDown(0); }
  }
  return { ids: chosen, features: features.size };
}

/**
 * What every sample holds: the programs rediscovery.json pins, and every row of the open-defects ledger (open-defects-rows.json), so a listed
 * row is judged at every seed and "a listed row stopped failing" stays a fact: a cover that moved off it would drop it as if fixed (74 rows at
 * smoke-p7, Part 16). The behaviour manifests are not required: they would take the sample from 12,000 to 18,400.
 */
export const requiredIds = ({ rows = ledgerIds() } = {}) => [...new Set([...readRediscovery().filter((entry) => !entry.outside).flatMap((entry) => entry.ids), ...rows])];
/** The ledger's ids, every grid's. */
export const ledgerIds = (path = OPEN_ROWS) => (existsSync(path) ? Object.values(JSON.parse(readFileSync(path, "utf8")).grids ?? {}).flatMap((rows) => Object.keys(rows)) : []);

/** One sample: the ids of each grid. `required`: the ids that come first, the others are chosen around them. */
export function sample({ perCode = false, seed = FIXED_SEED, required = [], grids = SAMPLED_GRIDS, codes } = {}) {
  const rows = grids.flatMap((grid) => gridRows(grid, { codes }));
  const { ids, features } = cover(rows, { perCode, seed, required });
  const of = new Map(rows.map((row) => [row.id, row.grid]));
  const byGrid = Object.fromEntries(grids.map((grid) => [grid, []]));
  for (const id of ids) byGrid[of.get(id)].push(id);
  return { byGrid, programs: ids.length, population: rows.length, features };
}

/** A sample as `--ids` manifests, one per grid, in `dir`: `{ grid: { path, ids } }`. */
export function writeSample({ perCode, seed, grids, runSeed, dir }) {
  const started = Date.now();
  const { byGrid, programs, population, features } = sample({ perCode, seed, grids, required: requiredIds() });
  mkdirSync(dir, { recursive: true });
  const written = {};
  for (const [grid, ids] of Object.entries(byGrid)) {
    const path = join(dir, `${grid}.json`);
    writeFileSync(path, `${JSON.stringify({ generator: GENERATOR_VERSION, seed: Number(runSeed), ids })}\n`);
    written[grid] = { path, ids };
  }
  console.log(`cover: ${perCode ? "per-code" : "all"} pairs, seed ${seed}: ${programs} of ${population} programs for ${features} pairs, chosen in ${Math.round((Date.now() - started) / 1000)}s`);
  return written;
}
