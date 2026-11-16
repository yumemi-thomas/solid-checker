// node scripts/guard/proved.mjs: a proved fuzz verdict is held against the merge base (ADR 1012). No checker build, no Cargo: it reads files at the base.
//
// scripts/fuzz/proved/<grid>.json is the ledger of the programs whose outcome was proved on main (`agree` on a misuse grid, `clean` on a legal one);
// the smoke and the audit fail on a ledger program that is another outcome now unless scripts/fuzz/proved/changes.json names it. A ledger and a
// naming file are only a ratchet if their change is compared with what it was, so:
//   1. changes.json is append-only: a row of the base that is gone or different is a failure. A NEW row needs an ADR in docs/decisions.md and a
//      `why`, and one shape: { grid, ids: [exact ids], adr, why } (the smoke's) or { grid, code, from, to, count, adr, why } (the audit's group:
//      `from` is the grid's proved outcome, `to` another, `count` the most losses it names). A row that names an id the ledger does not hold names nothing.
//   2. a ledger never loses an id: an id at the base that is not in the head's file of that grid is a failure (a lost proof stays in the ledger, named
//      by a row; nothing is ever forgotten by editing the file).
//   3. scripts/guard/corpus-changes.json (the corpus guard's naming file, scripts/guard/corpus.mjs) is append-only against the base, a new row with its shape and ADR.
//   4. scripts/fuzz/audit-known.json (the audit's known items, exact counts): a NEW entry, or one whose count rose against the base, needs an `adr`
//      in docs/decisions.md that the base's entry did not already name, and a `why`. A count may fall without one.
// No base is a failure, never a pass (COUNT_BASE=<rev> names one). A base with no scripts/fuzz/proved at all is the change that introduces it.
//
// Recording an intended change: append the changes row after adding the ADR; `node scripts/fuzz/proved.mjs record --from <run>` adds ids to a
// ledger; an audit-known entry is edited with its ADR.
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

import { baseCandidates, git, helpOf, repo } from "./base-revision.mjs";
import { isProved } from "../fuzz/proved.mjs";
import { CHANGES as CORPUS_CHANGES, changesProblems as corpusChangesProblems } from "./corpus.mjs";

const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);
const adrNumber = (adr) => /^(?:ADR )?(\d{4})$/.exec(adr ?? "")?.[1];
const text = (value) => typeof value === "string" && value.trim() !== "";
const inDecisions = (decisions, adr) => { const number = adrNumber(adr); return Boolean(number) && decisions.includes(`<a id="adr-${number}"></a>`); };

/** The problems of `changes.json` against its base: `head` and `base` are the parsed files, `ledgers` the head's `Map(grid -> Set(ids))`. */
export function changesProblems({ head, base, decisions, ledgers }) {
  const problems = [];
  for (const old of base.rows ?? []) if (!(head.rows ?? []).some((row) => same(row, old))) problems.push(`proved changes.json is append-only: a row of the base is gone or changed (${old.grid} ${old.adr ?? ""} ${(old.ids ?? [])[0] ?? old.code ?? ""})`);
  for (const row of head.rows ?? []) {
    if ((base.rows ?? []).some((old) => same(old, row))) continue;
    const name = `${row.grid ?? "?"} ${row.adr ?? "no ADR"}`;
    if (!text(row.grid)) problems.push(`proved changes.json: a new row names no grid (${name})`);
    if (!inDecisions(decisions, row.adr)) problems.push(`proved changes.json: new row ${name}: ${row.adr ?? "no adr"} is not an ADR in docs/decisions.md`);
    if (!text(row.why)) problems.push(`proved changes.json: new row ${name} says no why`);
    const byIds = Array.isArray(row.ids), byGroup = row.count !== undefined;
    if (byIds === byGroup) { problems.push(`proved changes.json: new row ${name} must name exact ids (ids) or a group (code, from, to, count), not both and not neither`); continue; }
    if (byIds) {
      if (!row.ids.length || row.ids.some((id) => !text(id)) || new Set(row.ids).size !== row.ids.length) problems.push(`proved changes.json: new row ${name}: ids must be a non-empty list of distinct ids`);
      const held = ledgers.get(row.grid) ?? new Set();
      const unknown = row.ids.filter((id) => !held.has(id));
      if (unknown.length) problems.push(`proved changes.json: new row ${name} names ${unknown.length} id(s) the ledger does not hold (${unknown.slice(0, 2).join(", ")}): it names nothing`);
    } else {
      if (!text(row.code)) problems.push(`proved changes.json: new group row ${name} names no code`);
      if (row.from !== undefined && !isProved(row.grid, row.from)) problems.push(`proved changes.json: new group row ${name}: from must be the grid's proved outcome (${isProved(row.grid, "agree") ? "agree" : "clean"})`);
      if (!text(row.to) || isProved(row.grid, row.to)) problems.push(`proved changes.json: new group row ${name}: to must be an outcome that is not proved`);
      if (!Number.isInteger(row.count) || row.count < 1) problems.push(`proved changes.json: new group row ${name}: count must be a positive integer`);
    }
  }
  return problems;
}

/** The ledgers against the base's: an id the base held and the head does not is a failure. `head`, `base`: `Map(grid -> Set(ids))`. */
export function ledgerProblems({ head, base }) {
  const problems = [];
  for (const [grid, ids] of base) {
    const now = head.get(grid);
    if (!now) { problems.push(`proved ledger of ${grid} is gone: a ledger never loses an id`); continue; }
    const lost = [...ids].filter((id) => !now.has(id));
    if (lost.length) problems.push(`proved ledger of ${grid} lost ${lost.length} id(s) (${lost.slice(0, 3).join(", ")}): a ledger only grows; a lost proof is named in changes.json`);
  }
  return problems;
}

const keyOf = (entry) => `${entry.grid}\0${entry.kind}\0${entry.item}`;
/** An entry's counts by recall mode: a number counts for both. */
const countsOf = (entry) => (typeof entry.count === "number" ? { lean: entry.count, full: entry.count } : { lean: entry.count?.lean ?? 0, full: entry.count?.full ?? 0 });

/** The audit's known list against the base: a new entry or a raised count needs a new ADR and a why. */
export function knownProblems({ head, base, decisions }) {
  const problems = [];
  const before = new Map((base.entries ?? []).map((entry) => [keyOf(entry), entry]));
  for (const entry of head.entries ?? []) {
    const old = before.get(keyOf(entry));
    const now = countsOf(entry), was = old ? countsOf(old) : null;
    if (old && now.lean <= was.lean && now.full <= was.full) continue;
    const name = `${entry.grid} ${entry.kind} ${entry.item}`;
    if (![now.lean, now.full].every((n) => Number.isInteger(n) && n >= 0)) problems.push(`audit-known.json: ${name}: count must be a non-negative integer (or { lean, full })`);
    if (!inDecisions(decisions, entry.adr)) problems.push(`audit-known.json: ${old ? "raised" : "new"} entry ${name}: ${entry.adr ?? "no adr"} is not an ADR in docs/decisions.md`);
    else if (old && old.adr === entry.adr) problems.push(`audit-known.json: entry ${name} rose (lean ${was.lean} -> ${now.lean}, full ${was.full} -> ${now.full}) under ${entry.adr}, the ADR the base already named: a rise needs a new ADR`);
    if (!text(entry.why)) problems.push(`audit-known.json: ${old ? "raised" : "new"} entry ${name} says no why`);
  }
  return problems;
}

const read = (file) => readFileSync(join(repo, file), "utf8");
const PROVED = "scripts/fuzz/proved";

/** The ledger files of a directory (`Map(grid -> Set(ids))`) and, at a revision, of `git ls-tree`. */
export function ledgersOf(files) {
  const map = new Map();
  for (const [name, content] of files) {
    if (name === "changes.json" || !name.endsWith(".json")) continue;
    const file = JSON.parse(content);
    map.set(file.grid, new Set(file.ids));
  }
  return map;
}

function main(argv) {
  if (argv.includes("--help")) return console.log(helpOf(import.meta.url));
  const { revisions, problem } = baseCandidates();
  if (problem) { console.error(problem); process.exitCode = 1; return; }
  const revision = revisions[0];
  const problems = [];
  const decisions = read("docs/decisions.md");
  const headFiles = existsSync(join(repo, PROVED)) ? readdirSync(join(repo, PROVED)).filter((name) => name.endsWith(".json")).map((name) => [name, read(`${PROVED}/${name}`)]) : [];
  const listing = git("ls-tree", "--name-only", revision, `${PROVED}/`);
  const baseNames = listing ? listing.split("\n").filter((name) => name.endsWith(".json")).map((name) => name.slice(PROVED.length + 1)) : [];
  const baseFiles = baseNames.map((name) => [name, git("show", `${revision}:${PROVED}/${name}`) ?? ""]);
  const headLedgers = ledgersOf(headFiles), baseLedgers = ledgersOf(baseFiles);
  const parse = (files, name) => { const found = files.find(([n]) => n === name); return found ? JSON.parse(found[1]) : { rows: [] }; };
  problems.push(...changesProblems({ head: parse(headFiles, "changes.json"), base: parse(baseFiles, "changes.json"), decisions, ledgers: headLedgers }));
  problems.push(...ledgerProblems({ head: headLedgers, base: baseLedgers }));
  // The corpus guard's naming file (scripts/guard/corpus.mjs) is held the same way here, where no corpus is needed: append-only, new rows with their ADR.
  const baseCorpus = git("show", `${revision}:${CORPUS_CHANGES}`);
  if (existsSync(join(repo, CORPUS_CHANGES))) problems.push(...corpusChangesProblems({ head: JSON.parse(read(CORPUS_CHANGES)), base: baseCorpus ? JSON.parse(baseCorpus) : { rows: [] }, decisions }));
  const known = "scripts/fuzz/audit-known.json";
  const baseKnown = git("show", `${revision}:${known}`);
  if (existsSync(join(repo, known))) problems.push(...knownProblems({ head: JSON.parse(read(known)), base: baseKnown ? JSON.parse(baseKnown) : { entries: [] }, decisions }));
  for (const line of problems) console.error(line);
  const ids = [...headLedgers.values()].reduce((sum, set) => sum + set.size, 0);
  console.log(`proved: ${ids} ledger ids in ${headLedgers.size} grids, compared with base ${revision}: ${problems.length} problem(s)`);
  process.exitCode = problems.length ? 1 : 0;
}

if (import.meta.main ?? process.argv[1] === fileURLToPath(import.meta.url)) main(process.argv.slice(2));
