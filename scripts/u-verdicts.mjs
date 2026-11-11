#!/usr/bin/env bun
// Which uncertifiable rows are legal code and which are misuse (ADR 0954): the typed blockers of `scripts/fuzz/blocker-scan.mjs` joined
// with the verdicts a grid run already holds, so a veto's cost reads as numbers per formula, not as a guess.
//
//   bun scripts/u-verdicts.mjs <rows.jsonl>... --verdicts <ids.tsv | ids.tsv.gz>... [--top 20] [--json]
//
// The verdicts are a run's per-program outcomes (`id<TAB>outcome<TAB>problem`, a header line first: the audit's `per-program/ids-<grid>.tsv.gz`).
// Chrome's verdict belongs to the program and the site, not to a checker build, so a table from an earlier build holds for every program the
// grid still generates; a program it does not list is `unjudged`, counted and never guessed. A scanned row (an uncertifiable finding with a
// `blockedBy`) is read against its program's outcome:
//   legal      a legal-grid program Chrome ran with no defect (`clean`, `uncertifiable`): lifting the blocker costs nothing
//   away       a row elsewhere in a confirmed misuse program: Chrome reported nothing there, as in legal code
//   blocking   a misuse program whose own outcome is `recall-gap` (Chrome confirmed the defect, the checker only declined), the row standing
//              in the judged site region: lifting it must turn the row into a violation; clean would be a false clean
//   shadowed   a misuse program with a violation at the site (`agree`, `stacked-claim`, ...), the row at the site: it blocks nothing
//   unjudged   no held verdict, or one that says nothing about the site (`typed-away`, `not-fired`, `not-legal`, ...)
// `misuse%` is blocking plus shadowed over the judged rows (every class but unjudged). Groups are the veto formulas: `(kind, formula, fired set)` as `scripts/u-blockers.mjs` keys them, and `(kind, missing fact)`. Every list is
// biggest first, ties by key: two runs and two machines print the same bytes. A scan of a sample (`--sample f`) is said so, and counts are
// the sample's: divide by the fraction for the grid's.
import { createReadStream, readFileSync } from "node:fs";
import { createInterface } from "node:readline";
import { gunzipSync } from "node:zlib";
import process from "node:process";
import { liftingSets } from "./u-blockers.mjs";

/** The classes, in the order the tables print them. */
export const CLASSES = ["legal", "away", "blocking", "shadowed", "unjudged"];

/** Outcomes of a legal-grid program that Chrome ran and found legal. */
const LEGAL_OUTCOMES = new Set(["clean", "uncertifiable"]);
/** Outcomes of a misuse program whose site the checker only declined (no violation at the site). */
const BLOCKED_OUTCOMES = new Set(["recall-gap"]);
/** Outcomes of a misuse program Chrome confirmed that stand a violation (or a claim of one) at the site. */
const CLAIMED_OUTCOMES = new Set(["agree", "wrong-rule", "stacked-claim", "halted-claim", "unplaced", "unjudged-claim", "misplaced", "false-clean"]);

/** The class of one scanned row: `side` is the program's grid side, `outcome` its held outcome (or undefined), `atSite` whether the row stands in the judged site region. */
export function rowClass(side, outcome, atSite) {
  if (outcome === undefined) return "unjudged";
  if (side === "legal") return LEGAL_OUTCOMES.has(outcome) ? "legal" : "unjudged";
  if (BLOCKED_OUTCOMES.has(outcome)) return atSite ? "blocking" : "away";
  if (CLAIMED_OUTCOMES.has(outcome)) return atSite ? "shadowed" : "away";
  return "unjudged";
}

/** `[a, b]` ordered by total (descending), then key. */
const byTotalThenKey = (a, b) => b.total - a.total || (a.key < b.key ? -1 : a.key > b.key ? 1 : 0);
const empty = () => Object.fromEntries(CLASSES.map((name) => [name, 0]));

/** A table of verdicts from text (`id<TAB>outcome<TAB>problem`, a header line first; blank lines ignored). */
export function readVerdicts(text, into = new Map()) {
  const lines = text.split("\n");
  for (let index = 1; index < lines.length; index += 1) {
    const line = lines[index];
    if (!line) continue;
    const [id, outcome] = line.split("\t");
    if (id && outcome) into.set(id, outcome);
  }
  return into;
}

/** A verdicts file's text: `.gz` is gunzipped. */
export const verdictText = (path) => (path.endsWith(".gz") ? gunzipSync(readFileSync(path)).toString("utf8") : readFileSync(path, "utf8"));

/**
 * The join of `programs` (parsed scan lines, the selection header first or absent) with `verdicts` (id to outcome): plain data, deterministic.
 * @returns {{ programs: number, errors: number, rows: number, unheld: number, selection: object[], formulas: object[], missing: object[], kinds: object[], classes: object }}
 */
export function join(programs, verdicts) {
  const formulas = new Map();
  const missing = new Map();
  const kinds = new Map();
  const total = empty();
  const selection = [];
  let count = 0;
  let errors = 0;
  let rows = 0;
  let unheld = 0;
  const bump = (map, key, mapKey, by = 1) => {
    if (!map.has(key)) map.set(key, { key, ...mapKey, total: 0, ...empty() });
    const entry = map.get(key);
    entry.total += by;
    return entry;
  };
  for (const program of programs) {
    if (program.scan) { selection.push(program.scan); continue; }
    count += 1;
    if (program.error) { errors += 1; continue; }
    const outcome = verdicts.get(program.id);
    if (outcome === undefined) unheld += 1;
    // Every scanned row by its kind: [at the site, away from it].
    for (const [kind, [atSite, away]] of Object.entries(program.u?.kinds ?? {})) {
      const here = rowClass(program.side, outcome, true);
      const there = rowClass(program.side, outcome, false);
      for (const [n, name] of [[atSite, here], [away, there]]) {
        if (!n) continue;
        bump(kinds, kind, { kind }, n)[name] += n;
        total[name] += n;
        rows += n;
      }
    }
    for (const row of program.formulas ?? []) {
      const name = rowClass(program.side, outcome, row.atSite);
      if (row.formula) {
        const key = JSON.stringify([row.kind, row.formula, row.fired]);
        bump(formulas, key, { kind: row.kind, formula: row.formula, fired: row.fired })[name] += 1;
      }
      if (row.missing) {
        const key = JSON.stringify([row.kind, row.missing.fact, row.missing.member ?? null]);
        bump(missing, key, { kind: row.kind, fact: row.missing.fact, member: row.missing.member ?? null })[name] += 1;
      }
    }
  }
  const sorted = (map) => [...map.values()].sort(byTotalThenKey);
  return {
    programs: count, errors, rows, unheld, selection,
    classes: total,
    formulas: sorted(formulas).map((group) => ({ ...group, lifting: liftingSets(group.formula, group.fired) })),
    missing: sorted(missing),
    kinds: sorted(kinds),
  };
}

const pad = (value, width) => String(value).padStart(width);
const percent = (part, whole) => (whole ? `${((100 * part) / whole).toFixed(1)}%` : "-");
/** The share of the judged rows that stand at a confirmed defect (blocking or shadowed). */
const misuse = (entry) => percent(entry.blocking + entry.shadowed, entry.total - entry.unjudged);
const header = () => `${pad("rows", 9)} ${CLASSES.map((name) => pad(name, 9)).join(" ")}  ${pad("misuse%", 8)}`;
const line = (entry) => `${pad(entry.total, 9)} ${CLASSES.map((name) => pad(entry[name], 9)).join(" ")}  ${pad(misuse(entry), 8)}`;

/** The report as text: the selection, the classes, the top `top` formula groups with their lifting alternatives, the missing facts, the kinds. */
export function render(result, { top = 20 } = {}) {
  const out = [];
  for (const scan of result.selection) {
    const part = [`grids ${scan.grids?.join(",")}`];
    if (scan.codes) part.push(`codes ${scan.codes.join(",")}`);
    if (scan.contexts) part.push(`contexts ${scan.contexts.join(",")}`);
    if (scan.sample !== null && scan.sample !== undefined) part.push(`SAMPLE ${scan.sample} (seed ${scan.sampleSeed}): counts are the sample's, the grid's are about ${Math.round(1 / scan.sample)} times these`);
    if (scan.shard) part.push(`shard ${scan.shard.join("/")}`);
    out.push(`scan: ${scan.programs} programs; ${part.join("; ")}`);
  }
  out.push(`${result.programs} programs read (${result.errors} errors, ${result.unheld} with no held verdict), ${result.rows} uncertifiable rows with a blocker kind`, "");
  out.push("classes (every row)", `  ${header()}`, `  ${line({ total: result.rows, ...result.classes })}`, "");
  const shown = result.formulas.slice(0, top);
  out.push(`formula groups (top ${shown.length} of ${result.formulas.length}; each \`lifts with\` is one way to lift the veto: every hazard on it must go)`, `  ${header()}  group`);
  for (const group of shown) {
    out.push(`  ${line(group)}  ${group.kind} ${group.formula} [${group.fired.join(", ")}]`);
    for (const set of group.lifting) out.push(`            lifts with: ${set.join(" + ")}`);
  }
  out.push("", `missing facts (${result.missing.length})`);
  if (result.missing.length === 0) out.push("  none: no emitter fills `missing` yet");
  else out.push(`  ${header()}  fact`);
  for (const group of result.missing.slice(0, top)) out.push(`  ${line(group)}  ${group.kind} ${group.fact}${group.member ? ` ${group.member}` : ""}`);
  out.push("", "blocker kinds (every row, formula or not)", `  ${header()}  kind`);
  for (const group of result.kinds) out.push(`  ${line(group)}  ${group.kind}`);
  return `${out.join("\n")}\n`;
}

/** The parsed lines of scan files, in the order given. */
export async function readScans(paths) {
  const programs = [];
  for (const path of paths) {
    const lines = createInterface({ input: createReadStream(path), crlfDelay: Infinity });
    for await (const text of lines) if (text) programs.push(JSON.parse(text));
  }
  return programs;
}

async function main(argv) {
  let top = 20;
  let json = false;
  const scans = [];
  const verdicts = [];
  for (let index = 0; index < argv.length; index += 1) {
    if (argv[index] === "--top") top = Number(argv[(index += 1)]);
    else if (argv[index] === "--json") json = true;
    else if (argv[index] === "--verdicts") verdicts.push(argv[(index += 1)]);
    else scans.push(argv[index]);
  }
  if (scans.length === 0 || verdicts.length === 0 || !Number.isInteger(top) || top < 1) {
    console.error("usage: bun scripts/u-verdicts.mjs <rows.jsonl>... --verdicts <ids.tsv[.gz]>... [--top 20] [--json]");
    process.exit(64);
  }
  const held = new Map();
  for (const path of verdicts) readVerdicts(verdictText(path), held);
  const result = join(await readScans(scans), held);
  process.stdout.write(json ? `${JSON.stringify({ ...result, formulas: result.formulas.slice(0, top) }, null, 2)}\n` : render(result, { top }));
}

if (import.meta.main) await main(process.argv.slice(2));
