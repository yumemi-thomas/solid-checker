#!/usr/bin/env node
// Do two runs of the fuzzer agree? The evidence that a speed change moved nothing it judges.
//
//   node scripts/fuzz/compare-runs.mjs <run dir A> <run dir B>      two `run.mjs --out` directories
//   node scripts/fuzz/compare-runs.mjs a/x.jsonl b/x.jsonl          two recorded results (results.jsonl, *.instrumented.jsonl)
//
// A directory is compared on: every results.jsonl row (all of it but the wall times), the verdict and item counts of summary.json,
// and findings.json (its groups, refuted candidates and false-clean groups, with the output directory taken off every path); a shrunk
// repro is named by the hash of its text (`repros/<sha1>`), so equal names are equal minima, and the repro files themselves are
// compared byte for byte. Exit status 0 when equal, 1 when not (with the first differences), 2 on a usage error.
//
//   --unexecuted-moves   a row recorded before the `candidate-unexecuted` verdict existed may differ from the same row after it in exactly one
//                        way: an `inconclusive` / `never-ran` item became a `candidate-unexecuted` item (the verdict follows). Those rows are
//                        listed as moves and do not make the runs different; anything else still does. Summary counts and findings are then
//                        not compared (the later run adds an `unexecuted` section the earlier one cannot have).
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const TIMINGS = new Set(["checkerMs", "runtimeMs"]);
const rowsOf = (path) => readFileSync(path, "utf8").split("\n").filter(Boolean).map((line) => JSON.parse(line));
const withoutTimings = (row) => Object.fromEntries(Object.entries(row).filter(([key]) => !TIMINGS.has(key)));

/** Rows by id that differ between two recorded results: `{ id, fields }` (a missing row has `fields: ["missing in A" | "missing in B"]`). */
export function diffRows(a, b) {
  const byId = (rows) => new Map(rows.map((row) => [row.id, withoutTimings(row)]));
  const left = byId(a); const right = byId(b);
  const out = [];
  for (const id of [...new Set([...left.keys(), ...right.keys()])].sort()) {
    if (!left.has(id)) { out.push({ id, fields: ["missing in A"] }); continue; }
    if (!right.has(id)) { out.push({ id, fields: ["missing in B"] }); continue; }
    const x = left.get(id); const y = right.get(id);
    const fields = [...new Set([...Object.keys(x), ...Object.keys(y)])].filter((key) => JSON.stringify(x[key]) !== JSON.stringify(y[key]));
    if (fields.length) out.push({ id, fields });
  }
  return out;
}

/** findings.json with a run's output directory taken off every path, so two runs' files compare. */
export function normalizeFindings(value, dir) {
  const prefix = resolve(dir);
  return JSON.parse(JSON.stringify(value, (_key, field) => (typeof field === "string" && field.startsWith(prefix) ? `<out>${field.slice(prefix.length)}` : field)));
}

/** Whether B's row is A's row with `inconclusive: never-ran` items turned into `candidate-unexecuted` ones, and nothing else changed. */
export function isUnexecutedMove(a, b) {
  const x = withoutTimings(a); const y = withoutTimings(b);
  const fields = [...new Set([...Object.keys(x), ...Object.keys(y)])].filter((key) => JSON.stringify(x[key]) !== JSON.stringify(y[key]));
  if (!fields.length || fields.some((key) => !["verdict", "reasons", "items"].includes(key))) return false;
  if (x.items.length !== y.items.length) return false;
  let moved = 0;
  for (const [index, left] of x.items.entries()) {
    const right = y.items[index];
    if (JSON.stringify(left) === JSON.stringify(right)) continue;
    const sameFinding = JSON.stringify(left.finding) === JSON.stringify(right.finding);
    if (!(sameFinding && left.kind === "inconclusive" && left.why === "never-ran" && right.kind === "candidate-unexecuted")) return false;
    moved += 1;
  }
  // The verdict follows the items: it stays, or an `inconclusive` program becomes a `candidate-unexecuted` one.
  return moved > 0 && (x.verdict === y.verdict || (x.verdict === "inconclusive" && y.verdict === "candidate-unexecuted"));
}

/** Rows that moved from `inconclusive: never-ran` to `candidate-unexecuted` and the rows that differ otherwise. */
export function partitionRows(a, b) {
  const left = new Map(a.map((row) => [row.id, row])); const right = new Map(b.map((row) => [row.id, row]));
  const moves = []; const other = [];
  for (const { id, fields } of diffRows(a, b)) {
    if (left.has(id) && right.has(id) && isUnexecutedMove(left.get(id), right.get(id))) moves.push(id); else other.push({ id, fields });
  }
  return { moves, other };
}

const filesUnder = (dir) => (existsSync(dir) ? readdirSync(dir, { recursive: true }).filter((name) => /\.(tsx|txt|json)$/.test(name)).sort() : []);

/** Compares two run directories; returns the list of differences (empty when equal). */
export function compareDirs(a, b) {
  const problems = [];
  const rows = diffRows(rowsOf(join(a, "results.jsonl")), rowsOf(join(b, "results.jsonl")));
  for (const { id, fields } of rows) problems.push(`row ${id}: ${fields.join(", ")}`);
  const summary = (dir) => { const { programs, verdicts, items, candidates, confirmed, groups, falseClean, unexecuted } = JSON.parse(readFileSync(join(dir, "summary.json"), "utf8")); return { programs, verdicts, items, candidates, confirmed, groups, falseClean, unexecuted }; };
  if (JSON.stringify(summary(a)) !== JSON.stringify(summary(b))) problems.push("summary.json counts differ");
  const findings = (dir) => { const { generator, seed, count, groups, refuted, falseClean, unexecuted } = normalizeFindings(JSON.parse(readFileSync(join(dir, "findings.json"), "utf8")), dir); return { generator, seed, count, groups, refuted, falseClean, unexecuted }; };
  const left = findings(a); const right = findings(b);
  for (const key of Object.keys(left)) if (JSON.stringify(left[key]) !== JSON.stringify(right[key])) problems.push(`findings.json ${key} differs`);
  const repros = (dir) => readdirSync(join(dir, "repros")).sort();
  if (existsSync(join(a, "repros")) || existsSync(join(b, "repros"))) {
    const ra = existsSync(join(a, "repros")) ? repros(a) : []; const rb = existsSync(join(b, "repros")) ? repros(b) : [];
    if (JSON.stringify(ra) !== JSON.stringify(rb)) problems.push(`repros differ: only in A ${ra.filter((n) => !rb.includes(n)).join(",") || "-"}; only in B ${rb.filter((n) => !ra.includes(n)).join(",") || "-"}`);
    for (const name of ra.filter((n) => rb.includes(n))) {
      const fa = filesUnder(join(a, "repros", name)); const fb = filesUnder(join(b, "repros", name));
      if (JSON.stringify(fa) !== JSON.stringify(fb) || fa.some((file) => readFileSync(join(a, "repros", name, file), "utf8") !== readFileSync(join(b, "repros", name, file), "utf8"))) problems.push(`repro ${name}: files differ`);
    }
  }
  return problems;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const argv = process.argv.slice(2);
  const moves = argv.includes("--unexecuted-moves");
  const [a, b] = argv.filter((arg) => !arg.startsWith("--"));
  if (!a || !b) { console.error("usage: compare-runs.mjs [--unexecuted-moves] <run dir A | results A> <run dir B | results B>"); process.exitCode = 2; }
  else if (moves) {
    const rowsA = rowsOf(a.endsWith(".jsonl") ? a : join(a, "results.jsonl")); const rowsB = rowsOf(b.endsWith(".jsonl") ? b : join(b, "results.jsonl"));
    const { moves: moved, other } = partitionRows(rowsA, rowsB);
    console.log(other.length ? `DIFFERENT (${other.length} row(s) differ beyond never-ran -> candidate-unexecuted; ${moved.length} moved):\n${other.slice(0, 40).map(({ id, fields }) => `row ${id}: ${fields.join(", ")}`).join("\n")}`
      : `EQUAL up to ${moved.length} never-ran -> candidate-unexecuted move(s) (${rowsA.length} programs)${moved.length ? `: ${moved.slice(0, 10).join(", ")}` : ""}`);
    process.exitCode = other.length ? 1 : 0;
  }
  else {
    const directories = !a.endsWith(".jsonl") && !b.endsWith(".jsonl");
    const problems = directories ? compareDirs(resolve(a), resolve(b)) : diffRows(rowsOf(a), rowsOf(b)).map(({ id, fields }) => `row ${id}: ${fields.join(", ")}`);
    const programs = directories ? rowsOf(join(a, "results.jsonl")).length : rowsOf(a).length;
    console.log(problems.length ? `DIFFERENT (${problems.length}):\n${problems.slice(0, 40).join("\n")}` : `EQUAL (${programs} programs${directories ? ", findings, summary and repros" : ""})`);
    process.exitCode = problems.length ? 1 : 0;
  }
}
