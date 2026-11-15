// node scripts/guard/base.mjs: ratchets that only the merge base can hold (ADR 0995). No checker build, no Cargo: it reads files at the base.
//
// A ledger, a cap or a frozen table is only a ratchet if its change is compared with what it was. Three of them were checked for their own
// consistency and never against the base, so the commit that loosened one could loosen its check in the same breath:
//
// 1. scripts/fuzz/judge-frozen.json. A row (rows, siteRows, legalRows, patternClaimRows) that is removed or whose content differs from the
//    base, and a table of the judge (`tables`) whose content differs, is a MOVED id. A moved id needs a NEW `changes` row (one the base
//    lacks) naming it: { id | table, adr, why, program } for a row `{ id, was, now, adr, why, program }`, for a table
//    `{ table, added, adr, why, program }`; the ADR is in docs/decisions.md and `program` names the Chrome program whose result shows the new
//    answer right. A row that is only ADDED is not moved. `changes` is append-only. (freeze-judge.mjs refuses to overwrite this file.)
// 2. fixtures/proof-log/candidates.json (the proof gate's false-clean candidates). It may only shrink: a key the base lacks is a NEW
//    candidate and needs `adr` (an ADR in docs/decisions.md) and `owner` (who fixes it). A candidate's entry that gets looser is not checked
//    here; a candidate that no hit matches is the gate's own stale failure.
// 3. The held cases of fixtures/uncertifiable-repro/cases.json and HELD_CAP (scripts/uncertifiable-causes.mjs). A case that is held now and
//    was not at the base (a new case, or an old one newly held) is a NEW held id: its `held.adr` ADR must be in docs/decisions.md, its text
//    must name the id, and the base's text of that ADR must not have (the ADR was written for this id, not an older one borrowed). HELD_CAP
//    may rise by no more than the new held ids.
//
// No base is a failure, never a pass (COUNT_BASE=<rev> names one). Exit 1 on any failure.
//
// Recording an intended change:
//   judge-frozen.json   append the `changes` row (above) in the same commit, after adding the ADR (docs/decisions.md) and the Chrome program;
//   candidates.json     give the new entry `adr` and `owner`, or fix the site and delete the entry;
//   held case           name its id in the ADR's section of docs/decisions.md and put that ADR's number in `held.adr`, then raise HELD_CAP by the count.
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

import { baseFile, helpOf, repo } from "./base-revision.mjs";

const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);
const adrNumber = (adr) => /^(?:ADR )?(\d{4})$/.exec(adr ?? "")?.[1];
const text = (value) => typeof value === "string" && value.trim() !== "";

/** The text of ADR `number`'s section in docs/decisions.md (its anchor to the next one), or null. */
export function adrSection(decisions, number) {
  const start = decisions.indexOf(`<a id="adr-${number}"></a>`);
  if (start < 0) return null;
  const next = decisions.indexOf('<a id="adr-', start + 1);
  return decisions.slice(start, next < 0 ? undefined : next);
}

const ROW_LISTS = ["rows", "siteRows", "legalRows", "patternClaimRows"];

/** Problems of a judge-frozen.json against its base: `head` and `base` are the parsed files. */
export function judgeFrozenProblems({ head, base, decisions }) {
  const problems = [];
  const added = (head.changes ?? []).filter((row) => !(base.changes ?? []).some((old) => same(old, row)));
  for (const old of base.changes ?? []) if (!(head.changes ?? []).some((row) => same(row, old))) problems.push(`judge-frozen changes is append-only: a row at the base is gone or changed (${old.id ?? old.table})`);
  const moved = [];
  for (const list of ROW_LISTS) {
    const now = new Map((head[list] ?? []).map((row) => [row.id, row]));
    for (const old of base[list] ?? []) {
      if (!now.has(old.id)) moved.push({ kind: "id", name: old.id, how: `row removed from ${list}` });
      else if (!same(now.get(old.id), old)) moved.push({ kind: "id", name: old.id, how: `row changed in ${list}` });
    }
  }
  for (const [table, content] of Object.entries(base.tables ?? {})) {
    if (!same(head.tables?.[table], content)) moved.push({ kind: "table", name: table, how: "table changed" });
  }
  for (const table of Object.keys(head.tables ?? {})) if (!(table in (base.tables ?? {}))) moved.push({ kind: "table", name: table, how: "table added" });
  for (const item of moved) {
    const row = added.find((candidate) => (item.kind === "id" ? candidate.id === item.name : candidate.table === item.name));
    if (!row) {
      problems.push(`judge-frozen: ${item.kind} ${item.name} moved (${item.how}) with no new \`changes\` row: add { ${item.kind === "id" ? "id, was, now" : "table, added"}, adr, why, program } naming an ADR and the Chrome program that shows the new answer right`);
      continue;
    }
    const number = adrNumber(row.adr);
    if (!number || !decisions.includes(`<a id="adr-${number}"></a>`)) problems.push(`judge-frozen: ${item.name}: ${row.adr ?? "no ADR"} is not in docs/decisions.md`);
    if (!text(row.program)) problems.push(`judge-frozen: ${item.name}: its changes row names no program`);
    if (!text(row.why)) problems.push(`judge-frozen: ${item.name}: its changes row says no why`);
  }
  return problems;
}

/** Problems of the proof gate's candidates against the base: a new key needs an ADR and an owner. */
export function candidateProblems({ head, base, decisions }) {
  const problems = [];
  for (const [key, entry] of Object.entries(head.candidates ?? {})) {
    if (key in (base.candidates ?? {})) continue;
    const number = adrNumber(entry.adr);
    if (!number || !decisions.includes(`<a id="adr-${number}"></a>`)) problems.push(`candidates.json: new candidate ${key}: ${entry.adr ?? "no adr"} is not an ADR in docs/decisions.md`);
    if (!text(entry.owner)) problems.push(`candidates.json: new candidate ${key}: no owner (who fixes it)`);
  }
  return problems;
}

/** The held ids of a cases.json object. */
export const heldIds = (cases) => new Map((cases?.cases ?? []).filter((entry) => entry.held).map((entry) => [entry.id, entry.held]));

/** `export const HELD_CAP = N` of an uncertifiable-causes.mjs text, or null. */
export const heldCap = (source) => {
  const found = /export const HELD_CAP = (\d+);/.exec(source ?? "");
  return found ? Number(found[1]) : null;
};

/** Problems of the held cases and HELD_CAP against the base. */
export function heldProblems({ head, base, headCap, baseCap, decisions, baseDecisions }) {
  const problems = [];
  const before = heldIds(base);
  const now = heldIds(head);
  const fresh = [...now].filter(([id]) => !before.has(id));
  for (const [id, held] of fresh) {
    const number = adrNumber(held.adr);
    const section = number ? adrSection(decisions, number) : null;
    if (!section) problems.push(`held case ${id}: held.adr ${held.adr ?? "(none)"} is not an ADR in docs/decisions.md`);
    else if (!section.includes(id)) problems.push(`held case ${id}: ADR ${number}'s section does not name the case: a new held case needs an ADR row that says why it is held`);
    else if (adrSection(baseDecisions, number)?.includes(id)) problems.push(`held case ${id}: ADR ${number} already named it at the base: a case newly held needs an ADR written for it`);
  }
  if (headCap === null || baseCap === null) problems.push("HELD_CAP is not readable in scripts/uncertifiable-causes.mjs at the head or the base");
  else if (headCap - baseCap > fresh.length) problems.push(`HELD_CAP rose ${baseCap} -> ${headCap} with ${fresh.length} new held case(s): the cap may rise by no more than the cases it admits`);
  return problems;
}

const read = (file) => readFileSync(join(repo, file), "utf8");

function main(argv) {
  if (argv.includes("--help")) return console.log(helpOf(import.meta.url));
  const problems = [];
  const decisions = read("docs/decisions.md");
  const counts = baseFile("docs/decisions.md");
  if (counts.problem) {
    console.error(counts.problem);
    process.exitCode = 1;
    return;
  }
  const baseDecisions = counts.text;
  const baseOf = (file) => {
    const found = baseFile(file);
    if (found.text === null) problems.push(found.problem);
    return found.text === null ? null : JSON.parse(found.text);
  };
  const frozen = "scripts/fuzz/judge-frozen.json";
  const candidates = "fixtures/proof-log/candidates.json";
  const cases = "fixtures/uncertifiable-repro/cases.json";
  const causes = "scripts/uncertifiable-causes.mjs";
  const baseFrozen = baseOf(frozen);
  if (baseFrozen) problems.push(...judgeFrozenProblems({ head: JSON.parse(read(frozen)), base: baseFrozen, decisions }));
  const baseCandidates = baseOf(candidates);
  if (baseCandidates) problems.push(...candidateProblems({ head: JSON.parse(read(candidates)), base: baseCandidates, decisions }));
  const baseCases = baseOf(cases);
  const baseCauses = baseFile(causes);
  if (baseCases && baseCauses.text !== null) problems.push(...heldProblems({ head: JSON.parse(read(cases)), base: baseCases, headCap: heldCap(read(causes)), baseCap: heldCap(baseCauses.text), decisions, baseDecisions }));
  else if (baseCauses.text === null) problems.push(baseCauses.problem);
  for (const problem of problems) console.error(problem);
  console.log(`base: judge-frozen, proof-gate candidates and held cases compared with base ${counts.revision}: ${problems.length} problem(s)`);
  process.exitCode = problems.length ? 1 : 0;
}

if (import.meta.main ?? process.argv[1] === fileURLToPath(import.meta.url)) main(process.argv.slice(2));
