#!/usr/bin/env bun
// Where the uncertifiable rows are blocked (ADR 0947): groups the `blockedBy` of a checker's internal JSON, or of a directory of them,
// and ranks the proofs the data asks for.
//
//   bun scripts/u-blockers.mjs <report.json | directory> [...] [--top 10] [--json]
//
// A checker report (`--format json --internal`) carries, on each uncertifiable finding, `blockedBy { kind, scope?, formula?, fired?,
// missing? }`, and a top-level `hazards: [{ id, sites }]` table the `fired` ids name. A veto is a formula over whole-program hazards, not
// one fact, so what the report derives is a set of *alternatives*: the minimal sets of present hazards whose removal lifts the veto (any
// one alternative suffices; every hazard on it must go). A hazard that is on no alternative is present but decides nothing, so it ranks
// no site. Everything is an upper bound: the first hole proven may expose the next.
//
// Output, in this order, every list biggest first and ties by key (no map order anywhere, so two runs and two machines agree):
//   kinds       rows by blocker kind, and the share with no located alternative (an alternative is located when every hazard on it has a site)
//   formulas    rows by (formula, fired set), each with its alternatives
//   hazards     per hazard on some alternative: its sites, the rows it is on an alternative of, and the rows it lifts alone
//   locations   the top sites of those hazards by the rows whose alternatives name the hazard (a row is counted once per site)
//   missing     rows by the typed fact that would decide them (`missing.fact`, `missing.member`)
import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import process from "node:process";

/** The reports under `paths`: a file is one, a directory contributes every `*.json` below it, sorted. */
export function reportFiles(paths) {
  const found = [];
  const walk = (path) => {
    if (statSync(path).isDirectory()) {
      for (const name of readdirSync(path).sort()) walk(join(path, name));
    } else if (path.endsWith(".json")) found.push(path);
  };
  for (const path of paths) walk(path);
  return found;
}

/** `[a, b]` ordered by count (descending), then key. */
const ranked = (counts) => [...counts].sort((a, b) => b[1] - a[1] || (a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : 0));
const bump = (map, key, by = 1) => map.set(key, (map.get(key) ?? 0) + by);

// The veto formulas over their atoms, restated from the checker (`base/contributions/blocked_by.rs`). A formula the report does not
// know lifts only when every fired atom goes.
const WRITTEN = /^(?:OpenMemberWrite\((?!<symbol>)|ReflectiveDefinitionSpelling$)/;
const SYMBOL_OPEN = ["StandardPatch(.Symbol)", "StandardPatch(.*)", "StandardImpostorWrite(Symbol)"];
const ITERATED_OPEN = /^(?:StandardImpostor\(|StandardOverride\(Array\.)/;

/** Whether the handed-root-patch formula holds over the atoms in `present`. */
export function handedRootHolds(present) {
  const has = (id) => present.has(id);
  const written =
    [...present].some((id) => WRITTEN.test(id)) || (has("OpenMemberWrite(<symbol>)") && SYMBOL_OPEN.some(has));
  const iteratedOpen = [...present].some((id) => ITERATED_OPEN.test(id));
  return (
    written &&
    (has("StandardRootHanded") ||
      ((has("StandardRootReceived") || has("StandardRootIterated")) && has("RootPatch")) ||
      (has("StandardRootIterated") && iteratedOpen))
  );
}

/**
 * The minimal subsets of `fired` whose removal lifts the veto of `formula`: every smaller set leaves it holding. Brute force over the
 * subsets (a row fires a dozen atoms at most), smallest first, at most `limit` sets.
 */
export function liftingSets(formula, fired, limit = 6) {
  const atoms = [...fired].sort();
  const holds =
    formula === "handed-root-patch"
      ? handedRootHolds
      : (present) => present.size > 0;
  if (!holds(new Set(atoms))) return [];
  if (atoms.length > 16) return [atoms];
  const found = [];
  const sizes = [...Array(atoms.length + 1).keys()];
  for (const size of sizes) {
    for (let mask = 1; mask < 1 << atoms.length; mask += 1) {
      if (popcount(mask) !== size) continue;
      const removed = atoms.filter((_, index) => (mask >> index) & 1);
      if (found.some((set) => set.every((id) => removed.includes(id)))) continue;
      if (!holds(new Set(atoms.filter((id) => !removed.includes(id))))) found.push(removed);
      if (found.length >= limit) return found;
    }
  }
  return found;
}

function popcount(value) {
  let count = 0;
  for (let rest = value; rest; rest >>= 1) count += rest & 1;
  return count;
}

/** `path:line` of a byte offset, or `path@byte` when the file is not on this machine. */
function placed(path, start, cache) {
  if (!cache.has(path)) cache.set(path, existsSync(path) ? readFileSync(path) : null);
  const bytes = cache.get(path);
  if (!bytes) return `${path}@${start}`;
  let line = 1;
  for (let index = 0; index < Math.min(start, bytes.length); index += 1) if (bytes[index] === 10) line += 1;
  return `${path}:${line}`;
}

/** The ways to lift a row's veto: its minimal lifting sets, else (a formula the script does not hold) every fired hazard together. */
function alternativesOf(formula, fired) {
  const sets = formula ? liftingSets(formula, fired) : [];
  if (sets.length > 0) return sets;
  return fired.length > 0 ? [[...fired].sort()] : [];
}

/** The analysis of `reports` (parsed checker JSON): plain data, deterministic. */
export function analyze(reports, { top = 10, source = new Map() } = {}) {
  const kinds = new Map();
  const unlocated = new Map();
  const formulas = new Map();
  const missing = new Map();
  const sites = new Map();
  const onAlternative = new Map();
  const alone = new Map();
  const siteSets = new Map();
  const alternatives = new Map();
  let rows = 0;
  for (const report of reports) {
    const table = new Map((report.hazards ?? []).map((hazard) => [hazard.id, hazard.sites ?? []]));
    for (const finding of report.findings ?? []) {
      if (finding.kind !== "uncertifiable" || !finding.blockedBy) continue;
      const blocked = finding.blockedBy;
      rows += 1;
      bump(kinds, blocked.kind);
      const fired = blocked.fired ?? [];
      const key = JSON.stringify([blocked.formula ?? null, [...fired].sort()]);
      if (!alternatives.has(key)) alternatives.set(key, alternativesOf(blocked.formula, fired));
      const ways = alternatives.get(key);
      const hasSites = (id) => (table.get(id) ?? []).length > 0;
      if (!ways.some((set) => set.every(hasSites))) bump(unlocated, blocked.kind);
      if (blocked.formula) bump(formulas, JSON.stringify([blocked.kind, blocked.formula, [...fired].sort()]));
      if (blocked.missing) bump(missing, JSON.stringify([blocked.kind, blocked.missing.fact, blocked.missing.member ?? null]));
      const named = new Set();
      for (const id of new Set(ways.flat())) {
        bump(onAlternative, id);
        for (const site of table.get(id) ?? []) {
          if (!siteSets.has(id)) siteSets.set(id, new Set());
          siteSets.get(id).add(`${site.path}\u0000${site.start}`);
          const siteKey = JSON.stringify([id, site.path, site.start]);
          if (named.has(siteKey)) continue;
          named.add(siteKey);
          bump(sites, siteKey);
        }
      }
      for (const set of ways) if (set.length === 1) bump(alone, set[0]);
    }
  }
  return {
    rows,
    kinds: ranked(kinds).map(([kind, count]) => ({ kind, rows: count, unlocated: unlocated.get(kind) ?? 0 })),
    formulas: ranked(formulas).map(([key, count]) => {
      const [kind, formula, fired] = JSON.parse(key);
      return { kind, formula, fired, rows: count, lifting: liftingSets(formula, fired) };
    }),
    hazards: ranked(onAlternative)
      .map(([id, count]) => ({ hazard: id, sites: siteSets.get(id)?.size ?? 0, rows: count, alone: alone.get(id) ?? 0 }))
      .sort((a, b) => b.alone - a.alone || b.rows - a.rows || (a.hazard < b.hazard ? -1 : a.hazard > b.hazard ? 1 : 0)),
    locations: ranked(sites)
      .slice(0, top)
      .map(([key, count]) => {
        const [id, path, start] = JSON.parse(key);
        return { hazard: id, site: placed(path, start, source), rows: count };
      }),
    missing: ranked(missing).map(([key, count]) => {
      const [kind, fact, member] = JSON.parse(key);
      return { kind, fact, member, rows: count };
    }),
  };
}

/** The report as text. */
export function render(result) {
  const out = [`uncertifiable rows with a blockedBy: ${result.rows}`, "", "kinds (rows, share with no located alternative)"];
  for (const row of result.kinds) out.push(`  ${String(row.rows).padStart(8)}  ${((100 * row.unlocated) / row.rows).toFixed(1).padStart(5)}%  ${row.kind}`);
  out.push("", "formulas (rows; fired set; each `lifts with` line is one way to lift the veto: every hazard on it must go)");
  for (const row of result.formulas) {
    out.push(`  ${String(row.rows).padStart(8)}  ${row.kind} ${row.formula} [${row.fired.join(", ")}]`);
    for (const set of row.lifting) out.push(`            lifts with: ${set.join(" + ")}`);
  }
  out.push("", "hazards on an alternative (sites; rows it is on an alternative of; rows it lifts alone, an upper bound)");
  for (const row of result.hazards) out.push(`  ${String(row.alone).padStart(8)} alone  ${String(row.rows).padStart(8)} rows  ${String(row.sites).padStart(5)} sites  ${row.hazard}`);
  out.push("", `locations (top ${result.locations.length}: sites of those hazards, by the rows whose alternatives name them)`);
  for (const row of result.locations) out.push(`  ${String(row.rows).padStart(8)}  ${row.site}  (${row.hazard})`);
  out.push("", "missing (rows by the typed fact that would decide them)");
  if (result.missing.length === 0) out.push("  none: no emitter fills `missing` yet");
  for (const row of result.missing) out.push(`  ${String(row.rows).padStart(8)}  ${row.kind} ${row.fact}${row.member ? ` ${row.member}` : ""}`);
  return `${out.join("\n")}\n`;
}

function main(argv) {
  let top = 10;
  let json = false;
  const paths = [];
  for (let index = 0; index < argv.length; index += 1) {
    if (argv[index] === "--top") top = Number(argv[(index += 1)]);
    else if (argv[index] === "--json") json = true;
    else paths.push(argv[index]);
  }
  if (paths.length === 0 || !Number.isInteger(top) || top < 1) {
    console.error("usage: bun scripts/u-blockers.mjs <report.json | directory> [...] [--top 10] [--json]");
    process.exit(64);
  }
  const reports = reportFiles(paths).map((path) => JSON.parse(readFileSync(path, "utf8")));
  const result = analyze(reports, { top });
  process.stdout.write(json ? `${JSON.stringify(result, null, 2)}\n` : render(result));
}

if (import.meta.main) main(process.argv.slice(2));
