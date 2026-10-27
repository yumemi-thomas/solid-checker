#!/usr/bin/env node
// The neutrality regression: do the counters move any verdict? (scripts/fuzz/README.md, "Neutrality")
//
// Runs one seed range twice through the same pipeline, once on the clean text with no hit ledger (the
// baseline: what the fuzzer did before the ledger) and once instrumented, and compares verdicts program by
// program. Only an `inconclusive` verdict may move, and only to a resolved one (the ledger turns "nothing
// shows it ran" into a verdict); any other change means the counters perturbed something, and exit status 1
// says so. It also reports: strip(runText) == text over the range, and how many findings that used to
// "span no site" (a finding on a plumbing line) now map to a site.
//
//   node scripts/fuzz/neutrality.mjs --seed 1 --count 1200 --out /tmp/neutral-1 \
//     --checker-bin <solid-checker> --typefacts <solid-typefacts> [--control]
//   node scripts/fuzz/neutrality.mjs --baseline a/results.jsonl --candidate b/results.jsonl   (compare two recorded runs)
//
// `--baseline <results.jsonl>` (alone) reuses a recorded baseline instead of re-running it; it must come
// from the same checker binary. `--control` also runs the clean twin and the checker on the twin's text.
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { compareVerdictRuns } from "./hits.mjs";
import { evaluate } from "./pipeline.mjs";
import { configFrom, generateEntries, parseArgs, resultRows } from "./run.mjs";

export const BLIND = "the finding spans no site of interest";

/** results.jsonl text -> rows. */
export const readRows = (path) => readFileSync(path, "utf8").split("\n").filter(Boolean).map((line) => JSON.parse(line));

const blindKeys = (rows) => rows.flatMap((row) => row.items.filter((item) => item.reason === BLIND && item.finding).map((item) => `${row.id}|${item.finding.rule}|${item.finding.line}`));

/**
 * How many findings the baseline could not place on a site (`BLIND`) are placed by the instrumented run.
 * A finding is identified by program, rule and line; "mapped" means the instrumented run has an item for it
 * that is not `BLIND`.
 */
export function blindFindingStats(baseRows, instrRows) {
  const before = blindKeys(baseRows);
  const stillBlind = new Set(blindKeys(instrRows));
  const outcome = new Map();
  for (const row of instrRows) for (const item of row.items) if (item.finding && item.reason !== BLIND) outcome.set(`${row.id}|${item.finding.rule}|${item.finding.line}`, item.kind);
  const mapped = before.filter((key) => !stillBlind.has(key) && outcome.has(key));
  const by = mapped.reduce((counts, key) => ({ ...counts, [outcome.get(key)]: (counts[outcome.get(key)] ?? 0) + 1 }), {});
  return { before: before.length, stillBlind: before.filter((key) => stillBlind.has(key)).length, mapped: mapped.length, mappedTo: by };
}

/** The full comparison of two runs' rows: verdict equality, resolved moves and the blind-finding statistics. */
export function compareRuns(baseRows, instrRows) {
  const verdicts = (rows) => new Map(rows.map((row) => [row.id, row.verdict]));
  const verdict = compareVerdictRuns(verdicts(baseRows), verdicts(instrRows));
  const control = instrRows.filter((row) => row.control);
  return {
    programs: verdict.total, sameVerdict: verdict.same, resolved: verdict.resolved, failures: verdict.failures, missing: verdict.missing, equal: verdict.equal,
    blind: blindFindingStats(baseRows, instrRows),
    control: control.length ? { programs: control.length, matched: control.filter((row) => row.control.matched).length, mismatches: control.filter((row) => !row.control.matched).map((row) => ({ id: row.id, diffs: row.control.diffs })) } : null,
  };
}

async function main(argv) {
  const options = parseArgs(argv, { extraValues: ["baseline", "candidate"] });
  const baselinePath = options.baseline;
  const candidatePath = options.candidate;
  let baseRows; let instrRows; let out = null;
  if (baselinePath && candidatePath) {
    baseRows = readRows(baselinePath);
    instrRows = readRows(candidatePath);
  } else {
    if (!options.out) throw new Error("--out <fresh directory> is required");
    out = resolve(options.out);
    if (existsSync(out)) throw new Error(`use a fresh --out path (${out} exists)`);
    mkdirSync(out, { recursive: true });
    const entries = generateEntries(options);
    const stripFailures = entries.filter(({ built }) => !built.neutral).map(({ built }) => built.id);
    console.error(`strip(runText) == text: ${entries.length - stripFailures.length}/${entries.length}${stripFailures.length ? ` (fails: ${stripFailures.slice(0, 5).join(", ")})` : ""}`);
    const run = async (name, extra) => {
      const config = { ...configFrom(options, join(out, name)), ...extra };
      const rows = resultRows(await evaluate(entries, config));
      writeFileSync(join(out, `${name}.jsonl`), rows.map((row) => JSON.stringify(row)).join("\n") + "\n");
      return rows;
    };
    baseRows = baselinePath ? readRows(baselinePath) : await run("baseline", { instrument: false, control: false, deadSite: false });
    instrRows = await run("instrumented", { instrument: true });
  }
  const report = compareRuns(baseRows, instrRows);
  if (out) writeFileSync(join(out, "neutrality.json"), `${JSON.stringify(report, null, 2)}\n`);
  console.log(JSON.stringify({ ...report, resolved: report.resolved.length, failures: report.failures.slice(0, 10), missing: report.missing.length }, null, 2));
  return report.equal ? 0 : 1;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    process.exitCode = await main(process.argv.slice(2));
  } catch (error) {
    console.error(`neutrality: ${error.message}`);
    process.exitCode = 2;
  }
}
