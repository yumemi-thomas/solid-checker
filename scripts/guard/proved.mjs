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
//   5. (ADR 1118) a proof change appends no row: a NEW row of changes.json is a fix's, `kind: "fix"`, with a `witness` (a program id, key or fixture
//      where Chrome contradicts the verdict the fix removes) and `collateral` (how many TRUE proofs the fix lost beside the false ones: a count, 0 if
//      none). A fix with collateral owes it: scripts/fuzz/proved/owed.json holds `{ adr, owed, why }` per ADR, a NEW entry only beside a new fix row of
//      its ADR, and `owed` may fall, never rise, and an entry never goes (it falls to 0); docs/open-work.md names each ADR that still owes with "owed".
//   6. (ADR 1118) the ledgers are keyed (`entries`: [key, id]; `ids`: unkeyed; `retired`: churned entries): a key, an id or a retired key held at the
//      base is held at the head (an entry may move to `retired`, an id into an entry), never dropped.
//   7. (ADR 1118) the textbook tier (scripts/fuzz/proved/textbook.json, scripts/fuzz/textbook.mjs): a program of the base stays; with the same
//      bytes (hash), it stays certified if it was and its uncertifiable keys (`u`) only shrink. New bytes are a re-admission, checked by textbook.mjs.
// No base is a failure, never a pass (COUNT_BASE=<rev> names one). A base with no scripts/fuzz/proved at all is the change that introduces it.
//
// Recording an intended change: append the changes row after adding the ADR; `node scripts/fuzz/proved.mjs record --from <run>` adds ids to a
// ledger; an audit-known entry is edited with its ADR.
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

import { baseCandidates, git, helpOf, repo } from "./base-revision.mjs";
import { NOT_LEDGERS, isProved, ledgerOf } from "../fuzz/proved.mjs";
import { CHANGES as CORPUS_CHANGES, changesProblems as corpusChangesProblems } from "./corpus.mjs";

const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);
const adrNumber = (adr) => /^(?:ADR )?(\d{4})$/.exec(adr ?? "")?.[1];
const text = (value) => typeof value === "string" && value.trim() !== "";
const inDecisions = (decisions, adr) => { const number = adrNumber(adr); return Boolean(number) && decisions.includes(`<a id="adr-${number}"></a>`); };

/** What a parsed ledger holds: every id (keyed, unkeyed, retired) and every key (entries and retired). */
const heldIds = (file) => new Set([...(file?.ids ?? []), ...(file?.entries?.values() ?? []), ...(file?.retired?.values() ?? [])]);
const heldKeys = (file) => new Set([...(file?.entries?.keys() ?? []), ...(file?.retired?.keys() ?? [])]);

/** The problems of `changes.json` against its base: `head` and `base` are the parsed files, `ledgers` the head's `Map(grid -> ledgerOf)`. */
export function changesProblems({ head, base, decisions, ledgers }) {
  const problems = [];
  for (const old of base.rows ?? []) if (!(head.rows ?? []).some((row) => same(row, old))) problems.push(`proved changes.json is append-only: a row of the base is gone or changed (${old.grid} ${old.adr ?? ""} ${(old.ids ?? [])[0] ?? old.code ?? ""})`);
  for (const row of head.rows ?? []) {
    if ((base.rows ?? []).some((old) => same(old, row))) continue;
    const name = `${row.grid ?? "?"} ${row.adr ?? "no ADR"}`;
    if (!text(row.grid)) problems.push(`proved changes.json: a new row names no grid (${name})`);
    if (!inDecisions(decisions, row.adr)) problems.push(`proved changes.json: new row ${name}: ${row.adr ?? "no adr"} is not an ADR in docs/decisions.md`);
    if (!text(row.why)) problems.push(`proved changes.json: new row ${name} says no why`);
    // ADR 1118: a proof only adds verdicts, so only a fix (a false verdict removed) may name a loss, with its Chrome witness and its collateral.
    if (row.kind !== "fix") problems.push(`proved changes.json: new row ${name} is not a fix's (kind: "fix"): a proof change appends no row (ADR 1118)`);
    if (!text(row.witness)) problems.push(`proved changes.json: new row ${name} names no witness (a program or fixture where Chrome contradicts the removed verdict)`);
    if (!Number.isInteger(row.collateral) || row.collateral < 0) problems.push(`proved changes.json: new row ${name}: collateral must be the count of true proofs it lost (an integer, 0 if none)`);
    const byIds = Array.isArray(row.ids) || Array.isArray(row.keys), byGroup = row.count !== undefined;
    if (byIds === byGroup) { problems.push(`proved changes.json: new row ${name} must name exact ids or keys (ids, keys) or a group (code, from, to, count), not both and not neither`); continue; }
    if (byIds) {
      const names = [...(row.ids ?? []), ...(row.keys ?? [])];
      if (!names.length || names.some((id) => !text(id)) || new Set(names).size !== names.length) problems.push(`proved changes.json: new row ${name}: ids and keys must be a non-empty list of distinct names`);
      const file = ledgers.get(row.grid);
      const ids = heldIds(file), keys = heldKeys(file);
      const unknown = [...(row.ids ?? []).filter((id) => !ids.has(id)), ...(row.keys ?? []).filter((key) => !keys.has(key))];
      if (unknown.length) problems.push(`proved changes.json: new row ${name} names ${unknown.length} id(s) or key(s) the ledger does not hold (${unknown.slice(0, 2).join(", ")}): it names nothing`);
    } else {
      if (!text(row.code)) problems.push(`proved changes.json: new group row ${name} names no code`);
      if (row.from !== undefined && !isProved(row.grid, row.from)) problems.push(`proved changes.json: new group row ${name}: from must be the grid's proved outcome (${isProved(row.grid, "agree") ? "agree" : "clean"})`);
      if (!text(row.to) || isProved(row.grid, row.to)) problems.push(`proved changes.json: new group row ${name}: to must be an outcome that is not proved`);
      if (!Number.isInteger(row.count) || row.count < 1) problems.push(`proved changes.json: new group row ${name}: count must be a positive integer`);
    }
  }
  return problems;
}

/**
 * The owed collateral (ADR 1118) against its base: `head`, `base` the parsed owed.json, `changes` the head's changes.json, `baseChanges` the base's,
 * `openWork` docs/open-work.md. A new entry needs a new fix row of its ADR; `owed` falls or stays; an entry never goes; an ADR that owes is in open-work.
 */
export function owedProblems({ head, base, changes, baseChanges, decisions, openWork }) {
  const problems = [];
  const newFixes = new Set((changes.rows ?? []).filter((row) => !(baseChanges.rows ?? []).some((old) => same(old, row)) && row.kind === "fix").map((row) => adrNumber(row.adr)).filter(Boolean));
  const before = new Map((base.rows ?? []).map((row) => [adrNumber(row.adr), row]));
  const after = new Map();
  for (const row of head.rows ?? []) {
    const number = adrNumber(row.adr);
    if (!number || !inDecisions(decisions, row.adr)) { problems.push(`proved owed.json: ${row.adr ?? "no adr"} is not an ADR in docs/decisions.md`); continue; }
    if (after.has(number)) problems.push(`proved owed.json: ADR ${number} has two entries`);
    after.set(number, row);
    if (!Number.isInteger(row.owed) || row.owed < 0) problems.push(`proved owed.json: ADR ${number}: owed must be a count (an integer, 0 when paid)`);
    if (!text(row.why)) problems.push(`proved owed.json: ADR ${number} says no why`);
    const old = before.get(number);
    if (!old && !newFixes.has(number)) problems.push(`proved owed.json: a new entry for ADR ${number} with no new fix row of that ADR in changes.json`);
    if (old && row.owed > old.owed) problems.push(`proved owed.json: ADR ${number} owes ${row.owed}, more than the base's ${old.owed}: owed collateral only falls`);
    if (row.owed > 0 && !openWork.split("\n").some((line) => line.includes(`ADR ${number}`) && /owed/i.test(line))) problems.push(`proved owed.json: ADR ${number} owes ${row.owed} and no line of docs/open-work.md names it with "owed"`);
  }
  for (const number of before.keys()) if (!after.has(number)) problems.push(`proved owed.json: the entry of ADR ${number} is gone (an entry falls to 0, it never goes)`);
  return problems;
}

/** The textbook tier against its base (ADR 1118): `head`, `base` the parsed textbook.json. */
export function textbookProblems({ head, base }) {
  const problems = [];
  for (const [name, was] of Object.entries(base.programs ?? {})) {
    const now = head.programs?.[name];
    if (!now) { problems.push(`proved textbook.json: ${name} is gone: a textbook program never leaves the tier`); continue; }
    if (now.hash !== was.hash) continue;
    if (was.certified && !now.certified) problems.push(`proved textbook.json: ${name} was certified and is not: a lost proof is named in changes.json, never recorded`);
    const added = (now.u ?? []).filter((key) => !(was.u ?? []).includes(key));
    if (added.length) problems.push(`proved textbook.json: ${name} gained ${added.length} uncertifiable key(s) (${added[0]}): the list only shrinks`);
  }
  return problems;
}

/** The ledgers against the base's: what the base held, the head holds (ADR 1012; keyed by ADR 1118). `head`, `base`: `Map(grid -> ledgerOf)`. */
export function ledgerProblems({ head, base }) {
  const problems = [];
  for (const [grid, was] of base) {
    const now = head.get(grid);
    if (!now) { problems.push(`proved ledger of ${grid} is gone: a ledger never loses an id`); continue; }
    const ids = heldIds(now), keys = heldKeys(now);
    const lostIds = [...was.ids].filter((id) => !ids.has(id));
    const lostKeys = [...heldKeys(was)].filter((key) => !keys.has(key));
    if (lostIds.length) problems.push(`proved ledger of ${grid} lost ${lostIds.length} id(s) (${lostIds.slice(0, 3).join(", ")}): a ledger only grows; a lost proof is named in changes.json`);
    if (lostKeys.length) problems.push(`proved ledger of ${grid} lost ${lostKeys.length} key(s) (${lostKeys.slice(0, 3).join(", ")}): an entry stays, or retires (the generator's churn); it never goes`);
  }
  return problems;
}

const keyOf = (entry) => `${entry.grid}\0${entry.kind}\0${entry.item}`;
const idSet = (entry, mode) => new Set(entry.ids?.[mode] ?? []);

/**
 * The audit's known list against the base. An entry names the exact ids it holds by recall mode (`ids: { lean, full }`). The only change that needs
 * no ADR is REMOVING ids (an entry that fell): a new entry, an id added to an entry, a changed `idRegex`, `cap`, grid, kind or item (the key) is a new
 * item, and needs an `adr` in docs/decisions.md that the base's entry did not already name, and a `why`.
 */
export function knownProblems({ head, base, decisions }) {
  const problems = [];
  const before = new Map((base.entries ?? []).map((entry) => [keyOf(entry), entry]));
  for (const entry of head.entries ?? []) {
    const old = before.get(keyOf(entry));
    const name = `${entry.grid} ${entry.kind} ${entry.item}`;
    for (const mode of ["lean", "full"]) {
      const ids = entry.ids?.[mode];
      if (!Array.isArray(ids) || ids.some((id) => !text(id)) || new Set(ids).size !== ids.length || ids.some((id, i) => i && ids[i - 1] > id)) problems.push(`audit-known.json: ${name}: ids.${mode} must be a sorted list of distinct ids`);
    }
    let reason = null;
    if (!old) reason = "new";
    else if (entry.idRegex !== old.idRegex) reason = "its idRegex changed";
    else if (Boolean(entry.cap) !== Boolean(old.cap)) reason = "its cap changed";
    else for (const mode of ["lean", "full"]) {
      const was = idSet(old, mode);
      const added = [...idSet(entry, mode)].filter((id) => !was.has(id));
      if (added.length) { reason = `${added.length} ${mode} id(s) added (${added.slice(0, 2).join(", ")})`; break; }
    }
    if (!reason) continue;
    if (!inDecisions(decisions, entry.adr)) problems.push(`audit-known.json: entry ${name} changed (${reason}): ${entry.adr ?? "no adr"} is not an ADR in docs/decisions.md`);
    else if (old && old.adr === entry.adr) problems.push(`audit-known.json: entry ${name} changed (${reason}) under ${entry.adr}, the ADR the base already named: any change but removing ids needs a new ADR`);
    if (!text(entry.why)) problems.push(`audit-known.json: entry ${name} changed (${reason}) and says no why`);
  }
  return problems;
}

const read = (file) => readFileSync(join(repo, file), "utf8");
const PROVED = "scripts/fuzz/proved";

/** The ledger files (`[name, content]`) as `Map(grid -> ledgerOf)`, with the problems of their form (another grid's name, a duplicate, a disorder: a hand edit). */
export function ledgersOf(files, problems = []) {
  const map = new Map();
  for (const [name, content] of files) {
    if (NOT_LEDGERS.has(name) || !name.endsWith(".json")) continue;
    const grid = name.replace(/\.json$/, "");
    try { map.set(grid, ledgerOf(JSON.parse(content), grid, `proved ledger ${name}`)); } catch (error) { problems.push(error.message); }
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
  const headLedgers = ledgersOf(headFiles, problems), baseLedgers = ledgersOf(baseFiles);
  const parse = (files, name) => { const found = files.find(([n]) => n === name); return found ? JSON.parse(found[1]) : { rows: [] }; };
  problems.push(...changesProblems({ head: parse(headFiles, "changes.json"), base: parse(baseFiles, "changes.json"), decisions, ledgers: headLedgers }));
  problems.push(...ledgerProblems({ head: headLedgers, base: baseLedgers }));
  const textbookOf = (files) => { const found = files.find(([n]) => n === "textbook.json"); return found ? JSON.parse(found[1]) : { programs: {} }; };
  problems.push(...textbookProblems({ head: textbookOf(headFiles), base: textbookOf(baseFiles) }));
  problems.push(...owedProblems({ head: parse(headFiles, "owed.json"), base: parse(baseFiles, "owed.json"), changes: parse(headFiles, "changes.json"), baseChanges: parse(baseFiles, "changes.json"), decisions, openWork: read("docs/open-work.md") }));
  // The corpus guard's naming file (scripts/guard/corpus.mjs) is held the same way here, where no corpus is needed: append-only, new rows with their ADR.
  const baseCorpus = git("show", `${revision}:${CORPUS_CHANGES}`);
  if (existsSync(join(repo, CORPUS_CHANGES))) problems.push(...corpusChangesProblems({ head: JSON.parse(read(CORPUS_CHANGES)), base: baseCorpus ? JSON.parse(baseCorpus) : { rows: [] }, decisions }));
  const known = "scripts/fuzz/audit-known.json";
  const baseKnown = git("show", `${revision}:${known}`);
  if (existsSync(join(repo, known))) problems.push(...knownProblems({ head: JSON.parse(read(known)), base: baseKnown ? JSON.parse(baseKnown) : { entries: [] }, decisions }));
  for (const line of problems) console.error(line);
  const ids = [...headLedgers.values()].reduce((sum, file) => sum + file.ids.size + file.entries.size, 0);
  console.log(`proved: ${ids} ledger ids in ${headLedgers.size} grids, compared with base ${revision}: ${problems.length} problem(s)`);
  process.exitCode = problems.length ? 1 : 0;
}

if (import.meta.main ?? process.argv[1] === fileURLToPath(import.meta.url)) main(process.argv.slice(2));
