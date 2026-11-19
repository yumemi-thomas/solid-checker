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
//
// KEYS (ADR 1118). A generator id names a cell and a shape, not a program: when the generator changes, the same id is another program, and the
// ledger read that as a lost proof (50 of the 132 losses of audit 44353494239). An entry is keyed by what was proved: the program's content
// (`programHash`: the bytes the checker and Chrome read) and the (code, site line) its verdict is about, `<hash>:<code>:<line>` (`provedKey`); the
// grid rows carry it (`key`, matrix.mjs and legal.mjs). A ledger file holds `entries` ([key, id] pairs, sorted by key), `ids` (entries written
// before the keys, matched by id until a run proves them under a key: `record` converts them) and `retired` ([key, id]: the program an entry names
// is no longer generated under its id, the generator's churn, kept and reported, never a loss). A run's row with an entry's key judges it; a row
// with the entry's id and another key is churn; no row at all, when the run was asked for every ledger program, is a loss as before.
import { createHash } from "node:crypto";
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

/**
 * The program's content hash: the bytes the checker and Chrome read (`built.files`, the packages it writes), so the same program has the same hash
 * whatever id the generator gives it and another program has another.
 */
export function programHash(built) {
  const files = Object.entries(built.files ?? {}).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0));
  const packages = Object.entries(built.packages ?? {}).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0)).map(([name, contents]) => [name, Object.entries(contents).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))]);
  return createHash("sha256").update(JSON.stringify([files, packages])).digest("hex").slice(0, 20);
}

/** The key of a proved verdict: the program's hash, the code and the site line its verdict is about. */
export const provedKey = (built, code, line) => `${programHash(built)}:${code}:${line}`;

const sortedPairs = (pairs, name) => {
  for (const [index, pair] of pairs.entries()) {
    if (!Array.isArray(pair) || pair.length !== 2 || typeof pair[0] !== "string" || typeof pair[1] !== "string") throw new Error(`${name}: an entry is not a [key, id] pair`);
    if (index && pairs[index - 1][0] >= pair[0]) throw new Error(`${name}: entries are not sorted by key, or repeat one`);
  }
  return new Map(pairs);
};

/** One ledger file's parsed form: `{ entries: Map(key -> id), ids: Set (unkeyed), retired: Map(key -> id) }`; throws on a hand edit. */
export function ledgerOf(file, grid, name = grid) {
  if (file.grid !== grid) throw new Error(`${name}: its grid is ${JSON.stringify(file.grid)}, the file's is ${grid}`);
  const ids = file.ids ?? [];
  if (new Set(ids).size !== ids.length) throw new Error(`${name}: duplicate ids`);
  if (ids.some((id, index) => index && ids[index - 1] > id)) throw new Error(`${name}: ids are not sorted`);
  return { entries: sortedPairs(file.entries ?? [], name), ids: new Set(ids), retired: sortedPairs(file.retired ?? [], name) };
}

/** Every grid's ledger file in `dir`, parsed: `Map(grid -> ledgerOf)`. */
export function readLedgerFiles(dir = PROVED_DIR) {
  const ledger = new Map();
  if (!existsSync(dir)) return ledger;
  for (const name of readdirSync(dir).sort()) {
    if (!name.endsWith(".json") || NOT_LEDGERS.has(name)) continue;
    const grid = name.replace(/\.json$/, "");
    ledger.set(grid, ledgerOf(JSON.parse(readFileSync(join(dir, name), "utf8")), grid, join(dir, name)));
  }
  return ledger;
}

/** The files of the proved directory that are not a grid's ledger. */
export const NOT_LEDGERS = new Set(["changes.json", "owed.json"]);

/** Every grid's ledger in `dir` as the ids it holds (keyed entries and unkeyed ids; not the retired): `Map(grid -> Set(ids))`. The cover asks for these programs. */
export function readLedger(dir = PROVED_DIR) {
  return new Map([...readLedgerFiles(dir)].map(([grid, file]) => [grid, new Set([...file.ids, ...file.entries.values()])]));
}

/** A ledger file's text: one entry per line, so a diff shows exactly what it adds. Only the parts with something in them are written. */
export function ledgerText(grid, ids, entries = new Map(), retired = new Map()) {
  const pairs = (map) => [...map].sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0)).map((pair) => JSON.stringify(pair)).join(",\n");
  const parts = [`"grid":${JSON.stringify(grid)}`];
  if (entries.size) parts.push(`"entries":[\n${pairs(entries)}\n]`);
  if (new Set(ids).size || (!entries.size && !retired.size)) parts.push(`"ids":[\n${[...new Set(ids)].sort().map((id) => JSON.stringify(id)).join(",\n")}\n]`);
  if (retired.size) parts.push(`"retired":[\n${pairs(retired)}\n]`);
  return `{${parts.join(",")}}\n`;
}

/** `{ rows: [...] }` of changes.json (none yet: an empty list). */
export function readChanges(path = CHANGES) {
  return existsSync(path) ? JSON.parse(readFileSync(path, "utf8")) : { rows: [] };
}

/** The ids and keys of `grid` that a changes row names (the smoke's exact naming: `ids`, or `keys` for a keyed entry). */
export function namedIds(changes, grid) {
  const named = new Set();
  for (const row of changes.rows ?? []) if (row.grid === grid) for (const name of [...(Array.isArray(row.ids) ? row.ids : []), ...(Array.isArray(row.keys) ? row.keys : [])]) named.add(name);
  return named;
}

/**
 * A run's moves against a grid's ledger. `ids`: the ledger's unkeyed ids of the grid, matched by id; `entries`: its keyed entries (`Map(key -> id)`),
 * matched by key; `rows`: the run's rows (`{ id, outcome, key }`); `expectAll`: the run was asked for every ledger program (a smoke's cover holds them
 * all), so one with no row is lost too (`to: "no row"`: the grid no longer generates the id, or its unit failed). `{ lost, unnamed, named, gained,
 * churn }`: `lost` every ledger program whose row is not proved, `unnamed` the ones no changes row names (by id, or by key for an entry), `gained`
 * the proved rows the ledger lacks, `churn` the entries whose id the run generated as another program (another key): not judged, not lost.
 */
export function provedMoves({ grid, ids = new Set(), entries = new Map(), rows, changes = { rows: [] }, expectAll = false }) {
  const byId = new Map(rows.map((row) => [row.id, row]));
  const byKey = new Map(rows.filter((row) => row.key).map((row) => [row.key, row]));
  const names = namedIds(changes, grid);
  const lost = [], churn = [];
  for (const id of ids) {
    const row = byId.get(id);
    if (row === undefined) {
      if (expectAll) lost.push({ id, to: "no row" });
    } else if (!isProved(grid, row.outcome)) lost.push({ id, to: row.outcome });
  }
  for (const [key, id] of entries) {
    const row = byKey.get(key);
    if (row) {
      if (!isProved(grid, row.outcome)) lost.push({ id, key, to: row.outcome });
    } else if (byId.has(id)) churn.push({ id, key, now: byId.get(id).key ?? null });
    else if (expectAll) lost.push({ id, key, to: "no row" });
  }
  const unnamed = lost.filter((move) => !names.has(move.id) && !(move.key && names.has(move.key)));
  const held = new Set([...ids, ...entries.keys()]);
  let gained = 0;
  for (const row of rows) if (isProved(grid, row.outcome) && !held.has(row.key ?? row.id) && !held.has(row.id)) gained += 1;
  return { lost, unnamed, named: lost.length - unnamed.length, gained, churn };
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

/**
 * `record`: add a run's proved programs to the ledgers in `dir`; `{ grid: { before, after, retired } }`. A proved row with a key becomes an entry; an
 * unkeyed id the run proved under a key becomes that entry (the id leaves `ids`); an entry whose id the run generated as another program is retired
 * (the generator's churn: kept, by key, never a loss). Nothing is ever removed: an id or key only moves to where it is held more exactly.
 */
export function record(from, dir = PROVED_DIR) {
  mkdirSync(dir, { recursive: true });
  const files = readLedgerFiles(dir);
  const counts = {};
  for (const grid of runGrids(from)) {
    const file = files.get(grid) ?? { entries: new Map(), ids: new Set(), retired: new Map() };
    const before = file.entries.size + file.ids.size;
    const ids = new Set(file.ids), entries = new Map(file.entries), retired = new Map(file.retired);
    const rows = readRows(join(from, grid));
    const byId = new Map(rows.map((row) => [row.id, row]));
    const keys = new Set(rows.map((row) => row.key).filter(Boolean));
    for (const [key, id] of file.entries) if (!keys.has(key) && byId.get(id)?.key && byId.get(id).key !== key) { entries.delete(key); retired.set(key, id); }
    for (const row of rows) {
      if (!isProved(grid, row.outcome)) continue;
      if (row.key) { entries.set(row.key, row.id); retired.delete(row.key); ids.delete(row.id); } else ids.add(row.id);
    }
    writeFileSync(ledgerPath(grid, dir), ledgerText(grid, ids, entries, retired));
    counts[grid] = { before, after: entries.size + ids.size, retired: retired.size };
  }
  return counts;
}

function main(argv) {
  const command = argv[0];
  const option = (name) => { const at = argv.indexOf(name); return at < 0 ? undefined : argv[at + 1]; };
  const from = option("--from"), dir = option("--dir") ? resolve(option("--dir")) : PROVED_DIR;
  if (!["record", "check"].includes(command) || !from) throw new Error("usage: proved.mjs record|check --from <run: one directory per grid> [--dir <ledger dir>]");
  if (command === "record") {
    for (const [grid, { before, after, retired }] of Object.entries(record(resolve(from), dir))) console.log(`${grid.padEnd(10)} ${String(before).padStart(6)} -> ${String(after).padStart(6)} proved programs (${retired} retired)`);
    return;
  }
  const ledger = readLedgerFiles(dir), changes = readChanges(join(dir, "changes.json"));
  let unnamed = 0;
  for (const grid of runGrids(resolve(from))) {
    const file = ledger.get(grid);
    if (!file) continue;
    const moves = provedMoves({ grid, ids: file.ids, entries: file.entries, rows: readRows(join(resolve(from), grid)), changes });
    unnamed += moves.unnamed.length;
    console.log(`${grid.padEnd(10)} ledger ${file.entries.size + file.ids.size}: lost ${moves.lost.length} (named ${moves.named}), newly proved ${moves.gained}, churn ${moves.churn.length}`);
    for (const line of provedLines(grid, moves)) console.log(`  ${line}`);
  }
  process.exitCode = unnamed ? 1 : 0;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main(process.argv.slice(2));
