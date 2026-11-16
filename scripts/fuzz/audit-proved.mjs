// The audit's proved-verdict ledger and its known items (ADR 1012).
//
// PROVED LEDGER. The audit compared a run with the latest earlier audit that wrote a report, green or not, and only counted what moved (1,791,685 ids
// moved at 78ddeb849 and none of it failed): a run red for nine days passed its losses on as the next baseline. The ledger of an audit is the set
// of programs it proved (`agree` on a misuse grid, `clean` on a legal one), kept as an artifact (`<dir>/<grid>.tsv.gz`: `id<TAB>code` lines; 1.2
// million ids, about 12 MB), and it advances only when the audit that produced it lost nothing unnamed:
//   - a program of the ledger that is not proved now is a loss; losses are grouped by (grid, code, to) and each group needs rows of
//     scripts/fuzz/proved/changes.json naming at least that many: { grid, code, from, to, count, adr, why } (or, for a program, { grid, ids, adr, why });
//   - a row names losses ONCE: the digest of every row an accepted audit used is kept in `consumed.json` beside the ledger, and a consumed row names
//     nothing in a later audit (a row cannot be reused against a later loss of its group);
//   - the new ledger is written only when every loss was named (`accepted`); otherwise the baseline stays the earlier one, and the run fails.
// Without a ledger the audit fails ("no proved ledger") unless it is told to bootstrap one (`--proved-bootstrap true`: a manual run on main, and the
// workflow refuses it when an audit-proved artifact of the mode ever existed). A bootstrap spends every row of changes.json that exists (their digests
// start `consumed`): a row written before the baseline cannot name a loss after it. A ledger that should exist (`--proved-expected true`: main has one)
// and cannot be read is attention, never "no ledger".
//
// KNOWN ITEMS. The audit's "needs a look" held the harness's and the judge's recorded classes next to what is new, so 872 lines were always red and a
// new line could not stand out. scripts/fuzz/audit-known.json lists a class with an exact count (`count`; `cap: true` for a class that varies with the
// runner's load, which may be lower, never higher): `{ grid, kind, item, idRegex, count, adr, why, work }`. The items an entry matches leave the
// attention list; more than the count are attention (the extra ones, by id); fewer than an exact count are attention too (the entry is stale: lower it).
import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { join } from "node:path";
import { gunzipSync, gzipSync } from "node:zlib";

import { isProved, provedOutcome } from "./proved.mjs";

const digest = (row) => createHash("sha256").update(JSON.stringify(row)).digest("hex").slice(0, 16);

/** The ledger of a directory: `{ grids: Map(grid -> Map(id -> code)), consumed: Set(digests) }`, or null when there is none. */
export function readAuditLedger(dir) {
  if (!dir || !existsSync(dir)) return null;
  const files = readdirSync(dir).filter((name) => name.endsWith(".tsv.gz"));
  if (!files.length) return null;
  const grids = new Map();
  for (const name of files) {
    const ids = new Map();
    for (const line of gunzipSync(readFileSync(join(dir, name))).toString("utf8").split("\n")) {
      if (!line) continue;
      const tab = line.indexOf("\t");
      ids.set(tab < 0 ? line : line.slice(0, tab), tab < 0 ? "-" : line.slice(tab + 1));
    }
    grids.set(name.replace(/\.tsv\.gz$/, ""), ids);
  }
  const consumed = existsSync(join(dir, "consumed.json")) ? new Set(JSON.parse(readFileSync(join(dir, "consumed.json"), "utf8"))) : new Set();
  return { grids, consumed };
}

/** Write the ledger of the proved programs of `rows` (`Map(grid -> Map(id -> outcome))`, codes `Map(grid -> Map(id -> code))`) and the consumed digests. */
export function writeAuditLedger(dir, rows, codes, consumed) {
  mkdirSync(dir, { recursive: true });
  for (const [grid, outcomes] of rows) {
    const lines = [];
    for (const [id, outcome] of outcomes) if (isProved(grid, outcome)) lines.push(`${id}\t${codes.get(grid)?.get(id) ?? "-"}`);
    lines.sort();
    writeFileSync(join(dir, `${grid}.tsv.gz`), gzipSync(`${lines.join("\n")}\n`));
  }
  writeFileSync(join(dir, "consumed.json"), `${JSON.stringify([...consumed].sort())}\n`);
}

/**
 * The audit's proved-verdict check. `rows`: `Map(grid -> Map(id -> outcome))`, `codes`: `Map(grid -> Map(id -> code))`, `ledger`: readAuditLedger's,
 * `changes`: changes.json's `{ rows }`. `{ attention, accepted, losses, consumed, gained }`: `accepted` when every loss was named; `consumed` the
 * digests the earlier ledger kept plus the rows this check used.
 */
export function auditProved({ rows, codes, ledger, changes = { rows: [] }, bootstrap = false, expected = false }) {
  if (!ledger) {
    if (expected) return { attention: ["proved ledger: main has one (the workflow found its artifact) and it could not be read: the run is not held to it and does not replace it"], accepted: false, losses: 0, consumed: new Set(), gained: 0 };
    // A bootstrap is a baseline from nothing: every row that exists is spent, so none names a later loss.
    if (bootstrap) return { attention: [], accepted: true, losses: 0, consumed: new Set((changes.rows ?? []).map(digest)), gained: 0, note: "no proved ledger: this run is the baseline (bootstrap)" };
    return { attention: ["no proved ledger: the audit has no baseline of its proved programs to hold this run to; a manual run on main with `proved-bootstrap` writes the first one"], accepted: false, losses: 0, consumed: new Set(), gained: 0 };
  }
  const attention = [];
  const used = new Set();
  const consumed = new Set(ledger.consumed);
  const usable = (changes.rows ?? []).map((row) => ({ row, digest: digest(row) })).filter(({ digest: d }) => !consumed.has(d));
  const available = new Map(); // `${grid}\0${code}\0${to}` -> [{ digest, left }]
  for (const { row, digest: d } of usable) {
    if (row.count === undefined) continue;
    const key = `${row.grid}\0${row.code}\0${row.to}`;
    available.set(key, [...(available.get(key) ?? []), { digest: d, left: row.count }]);
  }
  const named = new Map(); // grid -> Set(ids) of the exact-id rows
  for (const { row, digest: d } of usable) if (Array.isArray(row.ids)) { named.set(row.grid, new Set([...(named.get(row.grid) ?? []), ...row.ids])); used.add(d); }
  let total = 0, gained = 0, unnamedTotal = 0;
  for (const [grid, before] of ledger.grids) {
    const now = rows.get(grid);
    const groups = new Map(); // `${code}\0${to}` -> [ids]
    if (!now) { attention.push(`proved ledger: ${grid} is in the baseline but this run judged no program of it`); unnamedTotal += before.size; continue; }
    for (const [id, code] of before) {
      const outcome = now.get(id);
      if (outcome !== undefined && isProved(grid, outcome)) continue;
      total += 1;
      if (named.get(grid)?.has(id)) continue;
      const key = `${code}\0${outcome ?? "gone"}`;
      groups.set(key, [...(groups.get(key) ?? []), id]);
    }
    for (const [id, outcome] of now) if (isProved(grid, outcome) && !before.has(id)) gained += 1;
    for (const [key, ids] of [...groups].sort(([a], [b]) => (a < b ? -1 : 1))) {
      const [code, to] = key.split("\0");
      let missing = ids.length;
      for (const slot of available.get(`${grid}\0${code}\0${to}`) ?? []) {
        if (!missing) break;
        const take = Math.min(slot.left, missing);
        if (take > 0) { slot.left -= take; missing -= take; used.add(slot.digest); }
      }
      if (missing) {
        unnamedTotal += missing;
        attention.push(`${grid}: ${missing} proved verdict(s) of ${code} lost (${provedOutcome(grid)} -> ${to}), not named: e.g. ${ids.slice(0, 2).join(", ")}; a group row of scripts/fuzz/proved/changes.json { grid, code, from, to, count, adr, why } names them`);
      }
    }
  }
  for (const d of used) consumed.add(d);
  return { attention, accepted: unnamedTotal === 0, losses: total, consumed, gained };
}

/** The attention line kinds an audit item has: `{ grid, kind, id }` or null. */
export function itemOf(line) {
  const match = /^([a-z]+-[a-z]+): (unplaced|halted-claim|stacked-claim|false-clean|false-positive|oracle inconclusive|oracle differs) (\S+?):?(?: |$)/.exec(line);
  return match ? { grid: match[1], kind: match[2], id: match[3] } : null;
}

/** The ids an entry stores for a recall mode: `ids` is `{ lean: [...], full: [...] }`. */
export const idsFor = (entry, mode) => new Set(entry.ids?.[mode] ?? []);

/**
 * The attention lines with the known items taken out (audit-known.json `entries`): `{ attention, known }`. An entry names the exact ids it holds (`ids`, by
 * recall mode `mode`: lean or full) and matches the lines of its grid and kind whose id matches its `idRegex`. A matched id the entry stores leaves the
 * list; a matched id it does not store is attention (a new item, or one that took the place of a fixed one: the count alone would not show the swap);
 * a stored id no longer reported is attention too (a fix is news: remove it, `cap` entries excepted: they accept any subset of their ids).
 */
export function applyKnown(attention, known, mode = "full") {
  const entries = known?.entries ?? [];
  const taken = new Set();
  const out = [];
  const summary = [];
  for (const entry of entries) {
    const pattern = new RegExp(entry.idRegex);
    const stored = idsFor(entry, mode);
    const matched = [];
    attention.forEach((line, index) => {
      if (taken.has(index)) return;
      const item = itemOf(line);
      if (item && item.grid === entry.grid && item.kind === entry.kind && pattern.test(item.id)) matched.push({ index, id: item.id });
    });
    matched.sort((a, b) => (a.id < b.id ? -1 : a.id > b.id ? 1 : 0));
    const known = matched.filter((item) => stored.has(item.id));
    const extra = matched.filter((item) => !stored.has(item.id));
    for (const { index } of known) taken.add(index);
    const reported = new Set(matched.map((item) => item.id));
    const gone = [...stored].filter((id) => !reported.has(id)).sort();
    summary.push({ item: entry.item, grid: entry.grid, kind: entry.kind, known: known.length, count: stored.size, cap: Boolean(entry.cap) });
    const name = `${entry.grid} ${entry.kind} ${entry.item}`;
    if (extra.length) out.push(`known item ${name}: ${extra.length} id(s) it does not store (${stored.size} stored): ${extra.slice(0, 3).map((item) => item.id).join(", ")}${extra.length > 3 ? ", ..." : ""}`);
    if (gone.length && !entry.cap) out.push(`known item ${name}: ${gone.length} stored id(s) no longer reported (a fix is news; remove them from scripts/fuzz/audit-known.json): ${gone.slice(0, 3).join(", ")}${gone.length > 3 ? ", ..." : ""}`);
  }
  attention.forEach((line, index) => { if (!taken.has(index)) out.push(line); });
  // the lines of the ids an entry does not store stay: they are the items that stand out
  return { attention: out, known: summary };
}
