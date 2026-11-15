// The boundary twin (`focus.mjs --boundary both`, ADR 0990): the same programs judged under both program boundaries.
//
// A generated program has no manifest of its own, so the checker reads the install's: `{ "private": true }`, a CLOSED program. The closed-only proofs
// (the caller census of `exact_callers`, the closed-program discharge of an owner's entry, the module's exports an entry cannot reach) are therefore only ever
// checked where they are trusted. `--boundary open` gives every program a publishing manifest (checker.mjs `boundaryOverride`), the checker's other answer, and
// `--boundary both` runs the grids twice: closed first, then open, with each program's Chrome record taken from the runtime cache (the runtime does not read the
// manifest, so the record is the same under both). This module reads the two runs' rows side by side.
//
//   node scripts/fuzz/boundary-twin.mjs <closed run dir> <open run dir> [--json <file>]
//
// What each pair of outcomes means (matrix and legal rows; the row's own outcome is judged against that run's Chrome record):
//   closed-silent        the closed run's row is a false clean (Chrome fires, the checker certified or said nothing at the site) and the open run's is not:
//                        a closed-program premise is silent where the runtime fires. BLOCKER.
//   open-silent          the closed run's row is a violation (`agree`) and the open run's is a false clean: an open program lost the row and nothing replaced it. BLOCKER.
//   open-false-positive  a legal program the closed run left clean or uncertifiable gets a violation under open. BLOCKER.
//   open-lost-violation  the closed run's row is a violation and the open run's is uncertifiable (`recall-gap`): the open world hands the site off. Reported, a named cost.
//   open-gained          the open run's row is a violation where the closed run's is not.
//   open-moved           any other difference of outcome (counted by pair).
import { existsSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const SILENT = new Set(["false-clean"]);
const VIOLATION = new Set(["agree", "wrong-rule", "misplaced"]);
const HANDED = new Set(["recall-gap", "uncertifiable"]);
const SECONDARY_FALSE_CLEAN = (row) => Object.values(row.detail?.secondary ?? {}).includes("false-clean-certified");

/** What a pair of outcomes is (see the header): `null` for no difference. */
export function pairKind(closed, open) {
  if (closed.outcome === open.outcome && SECONDARY_FALSE_CLEAN(closed) === SECONDARY_FALSE_CLEAN(open)) return null;
  const silent = (row) => SILENT.has(row.outcome) || SECONDARY_FALSE_CLEAN(row);
  if (silent(closed) && !silent(open)) return "closed-silent";
  if (VIOLATION.has(closed.outcome) && silent(open)) return "open-silent";
  if (open.outcome === "false-positive" && closed.outcome !== "false-positive") return "open-false-positive";
  if (VIOLATION.has(closed.outcome) && HANDED.has(open.outcome)) return "open-lost-violation";
  if (!VIOLATION.has(closed.outcome) && VIOLATION.has(open.outcome)) return "open-gained";
  return "open-moved";
}

/** The kinds that are a false clean or a false positive: the twin's blockers. */
export const BLOCKER_KINDS = ["closed-silent", "open-silent", "open-false-positive"];

/** `{ pairs, transitions, kinds, missing }` of one grid unit's rows under each boundary (arrays of rows with `id` and `outcome`). */
export function compareUnit(closedRows, openRows) {
  const open = new Map(openRows.map((row) => [row.id, row]));
  const transitions = {};
  const kinds = {};
  const pairs = [];
  let missing = 0;
  for (const closed of closedRows) {
    const other = open.get(closed.id);
    if (!other) { missing += 1; continue; }
    const key = `${closed.outcome}->${other.outcome}`;
    transitions[key] = (transitions[key] ?? 0) + 1;
    const kind = pairKind(closed, other);
    if (!kind) continue;
    kinds[kind] = (kinds[kind] ?? 0) + 1;
    pairs.push({ id: closed.id, kind, closed: closed.outcome, open: other.outcome, closedDetail: closed.detail, openDetail: other.detail });
  }
  return { programs: closedRows.length, missing, transitions, kinds, pairs };
}

const readRows = (path) => readFileSync(path, "utf8").split("\n").filter(Boolean).map((line) => JSON.parse(line));

/** Every `programs.jsonl` under `dir`, by path relative to it (`mx-recall/programs.jsonl`, `mx-recall/CODE/programs.jsonl`). */
export function programFiles(dir) {
  const found = [];
  const visit = (at) => {
    for (const name of readdirSync(at).sort()) {
      if (name === "cases" || name === "chrome-cases" || name.startsWith(".")) continue;
      const path = join(at, name);
      if (statSync(path).isDirectory()) visit(path);
      else if (name === "programs.jsonl") found.push(relative(dir, path));
    }
  };
  if (existsSync(dir)) visit(dir);
  return found;
}

/** The twin's report over two run directories: per unit its comparison, and the blockers and named costs of the whole. */
export function compareRuns(closedDir, openDir) {
  const closedFiles = programFiles(closedDir);
  const openFiles = new Set(programFiles(openDir));
  const units = {};
  const problems = [];
  for (const file of closedFiles) {
    if (!openFiles.has(file)) { problems.push(`${file}: the open run wrote no rows for this unit`); continue; }
    units[file.replace(/\/programs\.jsonl$/, "")] = compareUnit(readRows(join(closedDir, file)), readRows(join(openDir, file)));
  }
  for (const file of openFiles) if (!closedFiles.includes(file)) problems.push(`${file}: the closed run wrote no rows for this unit`);
  const blockers = [];
  const costs = [];
  for (const [unit, result] of Object.entries(units)) {
    if (result.missing) problems.push(`${unit}: ${result.missing} programs of the closed run have no row in the open run`);
    for (const pair of result.pairs) (BLOCKER_KINDS.includes(pair.kind) ? blockers : pair.kind === "open-lost-violation" || pair.kind === "open-gained" ? costs : []).push({ unit, ...pair });
  }
  return { closedDir, openDir, units, blockers, costs, problems };
}

/** The report as lines: counts per unit and kind, then each blocker. */
export function reportLines(report) {
  const lines = [];
  for (const [unit, result] of Object.entries(report.units)) {
    const kinds = Object.entries(result.kinds).map(([kind, count]) => `${kind} ${count}`).join(", ") || "no difference";
    lines.push(`  ${unit.padEnd(24)} ${String(result.programs).padStart(6)} programs, closed vs open: ${kinds}`);
  }
  const moved = {};
  for (const result of Object.values(report.units)) for (const [key, count] of Object.entries(result.transitions)) moved[key] = (moved[key] ?? 0) + count;
  lines.push(`  outcome pairs (closed->open): ${Object.entries(moved).sort((a, b) => b[1] - a[1]).map(([key, count]) => `${key} ${count}`).join(", ")}`);
  for (const blocker of report.blockers) lines.push(`  BLOCKER ${blocker.kind} ${blocker.unit} ${blocker.id}: closed ${blocker.closed}, open ${blocker.open}`);
  if (report.costs.length) lines.push(`  named costs: ${report.costs.length} (${report.costs.filter((c) => c.kind === "open-lost-violation").length} violations the open world hands off, ${report.costs.filter((c) => c.kind === "open-gained").length} gained)`);
  for (const problem of report.problems) lines.push(`  PROBLEM ${problem}`);
  return lines;
}

function main(argv) {
  const [closedDir, openDir] = argv.filter((arg, index) => !arg.startsWith("--") && argv[index - 1] !== "--json");
  if (!closedDir || !openDir) { console.error("usage: boundary-twin.mjs <closed run dir> <open run dir> [--json <file>]"); return 2; }
  const report = compareRuns(resolve(closedDir), resolve(openDir));
  const json = argv.indexOf("--json");
  if (json >= 0) writeFileSync(resolve(argv[json + 1]), `${JSON.stringify(report, null, 1)}\n`);
  for (const line of reportLines(report)) console.log(line);
  return report.blockers.length || report.problems.length ? 1 : 0;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) process.exitCode = main(process.argv.slice(2));
