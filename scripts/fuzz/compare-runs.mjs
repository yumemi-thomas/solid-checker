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

const filesUnder = (dir) => (existsSync(dir) ? readdirSync(dir, { recursive: true }).filter((name) => /\.(tsx|txt|json)$/.test(name)).sort() : []);

/** Compares two run directories; returns the list of differences (empty when equal). */
export function compareDirs(a, b) {
  const problems = [];
  const rows = diffRows(rowsOf(join(a, "results.jsonl")), rowsOf(join(b, "results.jsonl")));
  for (const { id, fields } of rows) problems.push(`row ${id}: ${fields.join(", ")}`);
  const summary = (dir) => { const { programs, verdicts, items, candidates, confirmed, groups, falseClean } = JSON.parse(readFileSync(join(dir, "summary.json"), "utf8")); return { programs, verdicts, items, candidates, confirmed, groups, falseClean }; };
  if (JSON.stringify(summary(a)) !== JSON.stringify(summary(b))) problems.push("summary.json counts differ");
  const findings = (dir) => { const { generator, seed, count, groups, refuted, falseClean } = normalizeFindings(JSON.parse(readFileSync(join(dir, "findings.json"), "utf8")), dir); return { generator, seed, count, groups, refuted, falseClean }; };
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
  const [a, b] = process.argv.slice(2);
  if (!a || !b) { console.error("usage: compare-runs.mjs <run dir A | results A> <run dir B | results B>"); process.exitCode = 2; }
  else {
    const directories = !a.endsWith(".jsonl") && !b.endsWith(".jsonl");
    const problems = directories ? compareDirs(resolve(a), resolve(b)) : diffRows(rowsOf(a), rowsOf(b)).map(({ id, fields }) => `row ${id}: ${fields.join(", ")}`);
    const programs = directories ? rowsOf(join(a, "results.jsonl")).length : rowsOf(a).length;
    console.log(problems.length ? `DIFFERENT (${problems.length}):\n${problems.slice(0, 40).join("\n")}` : `EQUAL (${programs} programs${directories ? ", findings, summary and repros" : ""})`);
    process.exitCode = problems.length ? 1 : 0;
  }
}
