#!/usr/bin/env node
// The proved ledger's keys from its ids (ADR 1118): each unkeyed id of `scripts/fuzz/proved/<grid>.json` is generated again as its grid generates it
// (cover.mjs `gridRows`, blocker-scan.mjs `buildRow`: the generator at this tree, seed 1, the smoke's) and becomes the entry `[provedKey, id]`. No
// checker, no Chrome: the smoke judges a ledger id by the program the generator builds under it now, so the key of that program says exactly what the
// id said, and from here on the generator's churn (another program under the id) is told apart from a loss. An id the grid no longer generates stays
// unkeyed (the smoke reports it lost, "no row", as before).
//
//   node scripts/fuzz/proved-rekey.mjs [--dir scripts/fuzz/proved] [--grids lg-base,...]
import { writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { buildRow } from "./blocker-scan.mjs";
import { gridRows } from "./cover.mjs";
import { siteOf } from "./matrix.mjs";
import { PROVED_DIR, ledgerOf, ledgerPath, ledgerText, provedKey, readLedgerFiles } from "./proved.mjs";

/** Key the unkeyed ids of every grid in `dir` (or `grids`); `{ grid: { keyed, left } }`. */
export function rekey(dir = PROVED_DIR, grids = null) {
  const counts = {};
  for (const [grid, file] of readLedgerFiles(dir)) {
    if (grids && !grids.includes(grid)) continue;
    if (!file.ids.size) { counts[grid] = { keyed: 0, left: 0 }; continue; }
    const rows = new Map(gridRows(grid).filter((row) => file.ids.has(row.id)).map((row) => [row.id, row]));
    const ids = new Set(file.ids), entries = new Map(file.entries);
    for (const id of file.ids) {
      const row = rows.get(id);
      if (!row) continue;
      const built = buildRow(row);
      const key = provedKey(built, row.code, siteOf(built).line);
      if (entries.has(key) || file.retired.has(key)) throw new Error(`${grid}: ${id} keys to ${key}, which the ledger already holds for ${entries.get(key) ?? file.retired.get(key)}`);
      entries.set(key, id);
      ids.delete(id);
    }
    const text = ledgerText(grid, ids, entries, file.retired);
    ledgerOf(JSON.parse(text), grid); // the written form reads back
    writeFileSync(ledgerPath(grid, dir), text);
    counts[grid] = { keyed: file.ids.size - ids.size, left: ids.size };
  }
  return counts;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const argv = process.argv.slice(2);
  const option = (name) => { const at = argv.indexOf(name); return at < 0 ? undefined : argv[at + 1]; };
  const counts = rekey(option("--dir") ? resolve(option("--dir")) : PROVED_DIR, option("--grids")?.split(","));
  for (const [grid, { keyed, left }] of Object.entries(counts)) console.log(`${grid.padEnd(10)} keyed ${String(keyed).padStart(6)}, left unkeyed ${left}`);
}
