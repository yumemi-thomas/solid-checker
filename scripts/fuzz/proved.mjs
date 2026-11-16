// The proved-verdict ledger of the fuzz grids (ADR 1012): a program whose outcome was PROVED on main (`agree` on a misuse grid: the checker's
// violation matched Chrome's report; `clean` on a legal grid: the checker said nothing and Chrome ran it clean) cannot stop being proved without
// a row that names it. The smoke judged a program that moved from agree to recall-gap, or from clean to uncertifiable, as fine (focus.mjs treats
// recall-gap and uncertifiable as outcomes, `movedVerdicts` ranks them as no worse than attention), and the audit only counted the move: ADR
// 0997-0999-era merges lost proved violations that no run named.
//
//   node scripts/fuzz/proved.mjs record --from <run> [--dir scripts/fuzz/proved]
//       adds to each grid's ledger (`<dir>/<grid>.json`, sorted ids) the programs of a run (one directory per grid, each with programs.jsonl)
//       whose outcome is proved. It never removes an id: a ledger only grows (scripts/guard/proved.mjs holds that against the base).
//   node scripts/fuzz/proved.mjs check --from <run> [--dir ...]
//       the same comparison the smoke makes, on a run's directory: prints what was lost, what was named, what is newly proved.
//
// `changes.json` (append-only against the base) names a loss: { grid, ids: [exact ids], adr, why }. An audit loss is too big to name by id
// (an ADR can move thousands of ids of the 3.2 million programs), so the audit's rows name a group: { grid, code, from, to, count, adr, why }
// (audit.mjs; a row names its losses once: the accepted audit records the rows it used).
import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
export const PROVED_DIR = join(here, "proved");
export const CHANGES = join(PROVED_DIR, "changes.json");

/** The outcome that is a proof on a grid: `agree` on a misuse grid (mx-*), `clean` on a legal one (lg-*). */
export const provedOutcome = (grid) => (grid.startsWith("lg-") ? "clean" : "agree");
export const isProved = (grid, outcome) => outcome === provedOutcome(grid);

/** A grid's ledger file. */
export const ledgerPath = (grid, dir = PROVED_DIR) => join(dir, `${grid}.json`);

/** Every grid's ledger in `dir`: `Map(grid -> Set(ids))`; no directory is an empty ledger (the smoke then compares nothing). */
export function readLedger(dir = PROVED_DIR) {
  const ledger = new Map();
  if (!existsSync(dir)) return ledger;
  for (const name of readdirSync(dir).sort()) {
    if (name === "changes.json" || !name.endsWith(".json")) continue;
    const file = JSON.parse(readFileSync(join(dir, name), "utf8"));
    const grid = name.replace(/\.json$/, "");
    // A ledger file names its own grid and lists its ids sorted and distinct (`record` writes it so; scripts/guard/proved.mjs holds the same): another is a hand edit.
    if (file.grid !== grid) throw new Error(`${join(dir, name)}: its grid is ${JSON.stringify(file.grid)}, the file's is ${grid}`);
    const ids = file.ids ?? [];
    if (new Set(ids).size !== ids.length) throw new Error(`${join(dir, name)}: duplicate ids`);
    if (ids.some((id, index) => index && ids[index - 1] > id)) throw new Error(`${join(dir, name)}: ids are not sorted`);
    ledger.set(grid, new Set(ids));
  }
  return ledger;
}

/** A ledger file's text: sorted ids, one per line, so a diff shows exactly the ids it adds. */
export const ledgerText = (grid, ids) => `{"grid":${JSON.stringify(grid)},"ids":[\n${[...new Set(ids)].sort().map((id) => JSON.stringify(id)).join(",\n")}\n]}\n`;

/** `{ rows: [...] }` of changes.json (none yet: an empty list). */
export function readChanges(path = CHANGES) {
  return existsSync(path) ? JSON.parse(readFileSync(path, "utf8")) : { rows: [] };
}

/** The ids of `grid` that a changes row names (the smoke's exact naming). */
export function namedIds(changes, grid) {
  const named = new Set();
  for (const row of changes.rows ?? []) if (row.grid === grid && Array.isArray(row.ids)) for (const id of row.ids) named.add(id);
  return named;
}

/**
 * A run's moves against a grid's ledger. `ids`: the ledger's ids of the grid; `rows`: the run's rows (`{ id, outcome }`); `expectAll`: the run
 * was asked for every ledger id (a smoke's cover holds them all), so an id with no row is lost too (`to: "no row"`: the grid no longer
 * generates it, or its unit failed). `{ lost, unnamed, named, gained }`: `lost` every ledger id whose row is not proved, `unnamed` the ones no
 * changes row names, `gained` the proved rows the ledger lacks.
 */
export function provedMoves({ grid, ids, rows, changes = { rows: [] }, expectAll = false }) {
  const byId = new Map(rows.map((row) => [row.id, row.outcome]));
  const names = namedIds(changes, grid);
  const lost = [];
  for (const id of ids) {
    const outcome = byId.get(id);
    if (outcome === undefined) {
      if (expectAll) lost.push({ id, to: "no row" });
    } else if (!isProved(grid, outcome)) lost.push({ id, to: outcome });
  }
  const unnamed = lost.filter((move) => !names.has(move.id));
  let gained = 0;
  for (const [id, outcome] of byId) if (isProved(grid, outcome) && !ids.has(id)) gained += 1;
  return { lost, unnamed, named: lost.length - unnamed.length, gained };
}

/** The attention lines of a unit's unnamed losses: the first ten, then a count. */
export function provedLines(label, moves) {
  const lines = moves.unnamed.slice(0, 10).map((move) => `${label}: proved verdict lost ${move.id} -> ${move.to} (a row of scripts/fuzz/proved/changes.json with an ADR names it)`);
  if (moves.unnamed.length > 10) lines.push(`${label}: ... ${moves.unnamed.length - 10} more proved verdicts lost`);
  return lines;
}

const readRows = (dir) => readFileSync(join(dir, "programs.jsonl"), "utf8").split("\n").filter(Boolean).map((line) => JSON.parse(line));

/** The grids of a run directory: one subdirectory per grid, each with programs.jsonl. */
function runGrids(from) {
  return readdirSync(from, { withFileTypes: true }).filter((entry) => entry.isDirectory() && existsSync(join(from, entry.name, "programs.jsonl"))).map((entry) => entry.name).sort();
}

/** `record`: add a run's proved programs to the ledgers in `dir`; `{ grid: { before, after } }`. */
export function record(from, dir = PROVED_DIR) {
  mkdirSync(dir, { recursive: true });
  const ledger = readLedger(dir);
  const counts = {};
  for (const grid of runGrids(from)) {
    const before = ledger.get(grid) ?? new Set();
    const after = new Set(before);
    for (const row of readRows(join(from, grid))) if (isProved(grid, row.outcome)) after.add(row.id);
    writeFileSync(ledgerPath(grid, dir), ledgerText(grid, after));
    counts[grid] = { before: before.size, after: after.size };
  }
  return counts;
}

function main(argv) {
  const command = argv[0];
  const option = (name) => { const at = argv.indexOf(name); return at < 0 ? undefined : argv[at + 1]; };
  const from = option("--from"), dir = option("--dir") ? resolve(option("--dir")) : PROVED_DIR;
  if (!["record", "check"].includes(command) || !from) throw new Error("usage: proved.mjs record|check --from <run: one directory per grid> [--dir <ledger dir>]");
  if (command === "record") {
    for (const [grid, { before, after }] of Object.entries(record(resolve(from), dir))) console.log(`${grid.padEnd(10)} ${String(before).padStart(6)} -> ${String(after).padStart(6)} proved ids`);
    return;
  }
  const ledger = readLedger(dir), changes = readChanges(join(dir, "changes.json"));
  let unnamed = 0;
  for (const grid of runGrids(resolve(from))) {
    const ids = ledger.get(grid);
    if (!ids) continue;
    const moves = provedMoves({ grid, ids, rows: readRows(join(resolve(from), grid)), changes });
    unnamed += moves.unnamed.length;
    console.log(`${grid.padEnd(10)} ledger ${ids.size}: lost ${moves.lost.length} (named ${moves.named}), newly proved ${moves.gained}`);
    for (const line of provedLines(grid, moves)) console.log(`  ${line}`);
  }
  process.exitCode = unnamed ? 1 : 0;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main(process.argv.slice(2));
