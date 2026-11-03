#!/usr/bin/env node
// The evidence for demoting recall axis values (Fable's report 2026-11-03, R0.3): per program, does a recall mount's or a callback
// form's program answer as its twin without the axis (the same code, context, variant, binding and layer at the root, or with the
// callback written inline)? Counts alike are not enough (the report's own caveat): a value whose every program has its twin's outcome
// and the same findings, on a full run, adds nothing the base grid does not measure, and the lean recall grids (matrix.mjs LEAN_DEMOTED;
// run this on a `--recall-full` run, a lean one has no demoted value to check) may leave it out. One that differs anywhere keeps its place.
//
//   node scripts/fuzz/redundancy.mjs <run dir with mx-base/ and mx-recall/ (and lg-*)> [--out <redundancy.json>] [--against <mount>]
//     [--against-callback <form>]
// `--against-callback <form>` does the same for callback forms. `--against <mount>` pairs each recall mount's program with the same shape under that mount instead of the root one: whether two
// recall mounts answer alike (a value redundant with another, not with the base grid).
// A run dir as focus.mjs writes it: `<grid>/programs.jsonl`, or `<grid>/<unit>/programs.jsonl` with --split-codes.
import { existsSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

/** Every programs.jsonl row under a grid directory (one file, or one per code with --split-codes). */
export function gridRows(dir) {
  if (!existsSync(dir)) return [];
  const files = existsSync(join(dir, "programs.jsonl")) ? [join(dir, "programs.jsonl")] : readdirSync(dir).map((name) => join(dir, name, "programs.jsonl")).filter(existsSync);
  return files.flatMap((file) => readFileSync(file, "utf8").split("\n").filter(Boolean).map((line) => JSON.parse(line)));
}

/** A recall row's axis value and its twin's id: `-m-<mount>` and `-cb-<form>` cut from the id (matrix.mjs `mountSuffix`). */
export function twinOf(id, { against, againstCallback } = {}) {
  const mount = /-m-([a-z-]+)$/.exec(id);
  // `against`: a mount's twin is the same shape under that mount (are two mounts alike?), not the root program.
  if (mount && !id.includes("-s-")) return against ? (mount[1] === against ? null : { axis: `mount:${mount[1]}`, twin: `${id.slice(0, -mount[0].length)}-m-${against}` }) : { axis: `mount:${mount[1]}`, twin: id.slice(0, -mount[0].length) };
  const callback = /-cb-([a-z]+)$/.exec(id);
  // `againstCallback`: a callback form's twin is the same shape in that form, not the inline callback.
  if (callback) return againstCallback ? (callback[1] === againstCallback ? null : { axis: `callback:${callback[1]}`, twin: `${id.slice(0, -callback[0].length)}-cb-${againstCallback}` }) : { axis: `callback:${callback[1]}`, twin: id.slice(0, -callback[0].length) };
  return null;
}

/** What a row answers: its outcome and its findings at the site (detail.findings), line numbers left out (an axis moves lines). */
const answerOf = (row) => `${row.outcome}|${(row.detail?.findings ?? []).map((finding) => finding.replace(/@\d+$/, "")).sort().join(",")}`;

/** Per axis value: programs with a twin, alike, differing (with up to five examples), and programs whose twin the run lacks. */
export function redundancy(rows, { against, againstCallback } = {}) {
  const byId = new Map(rows.map((row) => [row.id, row]));
  const out = {};
  for (const row of rows) {
    const twin = twinOf(row.id, { against, againstCallback });
    if (!twin) continue;
    const slot = (out[twin.axis] ??= { programs: 0, alike: 0, differ: 0, noTwin: 0, examples: [] });
    slot.programs += 1;
    const other = byId.get(twin.twin);
    if (!other) { slot.noTwin += 1; continue; }
    if (answerOf(row) === answerOf(other)) slot.alike += 1;
    else { slot.differ += 1; if (slot.examples.length < 5) slot.examples.push(`${row.id}: ${answerOf(row)} vs ${answerOf(other)}`); }
  }
  return Object.fromEntries(Object.entries(out).sort(([a], [b]) => (a < b ? -1 : 1)));
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const [run, ...rest] = process.argv.slice(2);
  if (!run) { console.error("usage: node scripts/fuzz/redundancy.mjs <run dir> [--out <redundancy.json>] [--against <mount>]"); process.exit(2); }
  const report = {};
  const againstAt = rest.indexOf("--against");
  const againstMount = againstAt >= 0 ? rest[againstAt + 1] : undefined;
  const callbackAt = rest.indexOf("--against-callback");
  const againstCallback = callbackAt >= 0 ? rest[callbackAt + 1] : undefined;
  for (const [kind, grids] of [["misuse", ["mx-base", "mx-recall"]], ["legal", ["lg-base", "lg-recall"]]]) {
    const rows = grids.flatMap((grid) => gridRows(join(resolve(run), grid)));
    if (rows.length) report[kind] = redundancy(rows, { against: againstMount, againstCallback });
  }
  const at = rest.indexOf("--out");
  if (at >= 0) writeFileSync(rest[at + 1], `${JSON.stringify(report, null, 2)}\n`);
  for (const [kind, table] of Object.entries(report)) {
    console.log(`${kind}: axis value | programs | alike | differ | no twin`);
    for (const [axis, slot] of Object.entries(table)) console.log(`  ${axis.padEnd(24)} ${String(slot.programs).padStart(7)} ${String(slot.alike).padStart(7)} ${String(slot.differ).padStart(6)} ${String(slot.noTwin).padStart(6)}${slot.differ ? `  e.g. ${slot.examples[0]}` : ""}`);
  }
}
