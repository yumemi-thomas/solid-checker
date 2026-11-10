#!/usr/bin/env node
// The corpus delta of a veto or a fail-closed change, scoped to the projects it can touch (ADR 0955).
//
//   node scripts/corpus-delta.mjs --old <checker> --new <checker> [--typefacts <bin>] [--old-typefacts <bin>] [--new-typefacts <bin>]
//     (--hazard <id[,id...]> | --blocker <kind[,kind...]> | --all)
//     [--baseline <old build's corpus-sweep report>] [--controls 3]
//     [--manifest scripts/corpus/rc21-manifest.json] [--apps-root ../solid-checker-rc21-frozen/apps] [--apps a,b]
//     [--jobs 4] [--timeout 900] [--program-boundary open|closed] [--out <report.json>] [--json]
//
// A veto reads whole-program hazards (ADR 0947), so a change to one moves rows only in the projects where those hazards are present. The
// `hazards` table of the OLD build's report says which: a project is affected when its table holds a hazard `--hazard` names (an id, or its
// head: `OpenMemberWrite` is `OpenMemberWrite(*)` and `OpenMemberWrite(key)`), or a row's `blockedBy.kind` is one of `--blocker`. The old
// results are `--baseline` (a sweep of the old build: `corpus-sweep.mjs` keeps each project's `hazards` and findings), or, without one, a run
// of the old checker over every project here. The new checker runs on the affected projects only, and on `--controls` unaffected ones (the
// most uncertifiable rows first, ties by name), which must not move: one that does means the scope missed a hazard, and the run fails.
//   Scope is only as good as the table. A hazard the old build records no row for does not appear in its table, so a change that adds a NEW
//   veto, or a fail-closed result that reads no hazard, names `--all` (the whole corpus, as a sweep and `corpus-sweep-compare.mjs` would).
//   A project the baseline has no table for is affected: unknown is in.
//
// The report, deterministic (sorted, no clock): the builds and the scope; per project the violations and uncertifiable rows old to new;
// per blocker kind the uncertifiable rows old to new; the moves by rule and kind; and what needs a look, which fails the run: a
// violation the old build did not have (added, or a row that turned from uncertifiable to violation), a control that moved, a project the
// checker could not answer. A violation that left, or turned uncertifiable, is listed but is the change's purpose when it adds a veto.
import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

import { assertManifestProvenance } from "./lib/corpus-manifest.mjs";

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "..");

/** The blocker a finding names: its typed `blockedBy.kind` (ADR 0947), else the head of its analysis context, else `none`. */
export const blockerOf = (finding) => finding.blockedBy?.kind ?? (finding.analysisContext ? String(finding.analysisContext).split(":")[0] : "none");

/** Whether hazard `id` is one `wanted` (an exact id, or the head of one: the part before `(`) names. */
export const hazardMatches = (id, wanted) => wanted.some((name) => id === name || id.startsWith(`${name}(`));

/** `{ app, project }` as one key. */
export const keyOf = (row) => `${row.app}\u0000${row.project}`;

/**
 * Whether the old build records hazards at all: some project has a table or a row with a typed blocker (ADR 0947). A project with no
 * hazard omits its table, so a missing table says "none" of a build that records them and "unknown" of one that does not.
 */
export const recordsHazards = (results) => [...results.values()].some((result) => (Array.isArray(result.hazards) && result.hazards.length > 0) || (result.findings ?? []).some((finding) => finding.blockedBy));

/** The projects `spec` makes affected, among `results` (old build's, by key): `{ affected: Map<key, why>, unaffected: key[] }`. */
export function scope(results, spec, known = recordsHazards(results)) {
  const affected = new Map();
  const unaffected = [];
  for (const [key, result] of results) {
    let why = null;
    if (spec.all) why = "all";
    else {
      if (spec.hazards.length > 0) {
        if (!Array.isArray(result.hazards) && !known) why = "the old build records no hazards table";
        else {
          const hit = (result.hazards ?? []).map((hazard) => hazard.id).filter((id) => hazardMatches(id, spec.hazards)).sort();
          if (hit.length > 0) why = `hazards ${hit.join(", ")}`;
        }
      }
      if (why === null && spec.blockers.length > 0) {
        const kinds = [...new Set((result.findings ?? []).filter((finding) => finding.kind === "uncertifiable").map(blockerOf))].filter((kind) => spec.blockers.includes(kind)).sort();
        if (kinds.length > 0) why = `blockers ${kinds.join(", ")}`;
      }
    }
    if (why === null) unaffected.push(key);
    else affected.set(key, why);
  }
  return { affected, unaffected };
}

/** The unaffected projects to run the new build on as controls: the `count` with the most uncertifiable rows in the old report, ties by key. */
export function pickControls(results, unaffected, count) {
  const rows = (key) => (results.get(key)?.findings ?? []).filter((finding) => finding.kind === "uncertifiable").length;
  return [...unaffected].sort((a, b) => rows(b) - rows(a) || (a < b ? -1 : a > b ? 1 : 0)).slice(0, count);
}

const findingKey = (finding) => {
  const at = finding.primaryLocation ?? {};
  return `${finding.id}\u0000${at.path}\u0000${at.startByte}\u0000${at.endByte}`;
};

/**
 * One project's old and new findings: counts of violations and uncertifiable rows, the uncertifiable rows per blocker kind, and the moves
 * by finding key (id, path, span): a row that is new, gone, or changed kind.
 */
export function diffProject(oldFindings, newFindings) {
  const count = (findings) => {
    const out = { violation: 0, uncertifiable: 0, other: 0, blockers: {} };
    for (const finding of findings) {
      if (finding.kind === "violation") out.violation += 1;
      else if (finding.kind === "uncertifiable") {
        out.uncertifiable += 1;
        const kind = blockerOf(finding);
        out.blockers[kind] = (out.blockers[kind] ?? 0) + 1;
      } else out.other += 1;
    }
    return out;
  };
  const was = new Map();
  for (const finding of oldFindings) {
    const key = findingKey(finding);
    if (!was.has(key)) was.set(key, []);
    was.get(key).push(finding);
  }
  const now = new Map();
  for (const finding of newFindings) {
    const key = findingKey(finding);
    if (!now.has(key)) now.set(key, []);
    now.get(key).push(finding);
  }
  const moves = [];
  const kindsOf = (list) => list.map((finding) => finding.kind).sort().join("+");
  for (const [key, before] of was) {
    const after = now.get(key);
    const rule = before[0].id;
    if (!after) for (const finding of before) moves.push({ rule, from: finding.kind, to: "gone", blocker: blockerOf(finding), line: finding.primaryLocation?.line ?? null });
    else if (kindsOf(before) !== kindsOf(after)) moves.push({ rule, from: kindsOf(before), to: kindsOf(after), blocker: blockerOf(after[0]), line: after[0].primaryLocation?.line ?? null });
  }
  for (const [key, after] of now) {
    if (!was.has(key)) for (const finding of after) moves.push({ rule: finding.id, from: "new", to: finding.kind, blocker: blockerOf(finding), line: finding.primaryLocation?.line ?? null });
  }
  moves.sort((a, b) => (a.rule < b.rule ? -1 : a.rule > b.rule ? 1 : a.from < b.from ? -1 : a.from > b.from ? 1 : a.to < b.to ? -1 : a.to > b.to ? 1 : a.blocker < b.blocker ? -1 : a.blocker > b.blocker ? 1 : (a.line ?? 0) - (b.line ?? 0)));
  return { old: count(oldFindings), new: count(newFindings), moves };
}

/** Whether a move gives the project a violation it did not have. */
export const addsViolation = (move) => move.to.split("+").includes("violation") && !move.from.split("+").includes("violation");

/** The report data from the old and new results (by key), the scope and the controls. Plain data, deterministic. */
export function buildReport({ oldResults, newResults, affected, controls, errors = [] }) {
  const projects = [];
  const blockers = new Map();
  const moves = new Map();
  const needsLook = [];
  const totals = { old: { violation: 0, uncertifiable: 0 }, new: { violation: 0, uncertifiable: 0 } };
  for (const key of [...newResults.keys()].sort()) {
    const before = oldResults.get(key);
    const after = newResults.get(key);
    const [app, project] = key.split("\u0000");
    const delta = diffProject(before.findings ?? [], after.findings ?? []);
    const control = controls.includes(key);
    projects.push({ app, project, why: control ? "control" : affected.get(key), control, old: { violation: delta.old.violation, uncertifiable: delta.old.uncertifiable }, new: { violation: delta.new.violation, uncertifiable: delta.new.uncertifiable }, moved: delta.moves.length });
    for (const side of ["old", "new"]) for (const name of ["violation", "uncertifiable"]) totals[side][name] += delta[side][name];
    for (const kind of new Set([...Object.keys(delta.old.blockers), ...Object.keys(delta.new.blockers)])) {
      const entry = blockers.get(kind) ?? { kind, old: 0, new: 0 };
      entry.old += delta.old.blockers[kind] ?? 0;
      entry.new += delta.new.blockers[kind] ?? 0;
      blockers.set(kind, entry);
    }
    for (const move of delta.moves) {
      const moveKey = JSON.stringify([move.rule, move.from, move.to, move.blocker]);
      const entry = moves.get(moveKey) ?? { rule: move.rule, from: move.from, to: move.to, blocker: move.blocker, rows: 0, projects: new Set() };
      entry.rows += 1;
      entry.projects.add(`${app}/${project}`);
      moves.set(moveKey, entry);
      if (addsViolation(move)) needsLook.push(`${app}/${project}: ${move.rule} ${move.from} -> ${move.to}${move.line ? ` at line ${move.line}` : ""} (${move.blocker})`);
    }
    if (control && delta.moves.length > 0) needsLook.push(`control ${app}/${project} moved ${delta.moves.length} rows: the scope missed a hazard`);
  }
  for (const error of errors) needsLook.push(error);
  const byTotal = (a, b) => b.rows - a.rows || (JSON.stringify(a) < JSON.stringify(b) ? -1 : 1);
  return {
    projects,
    totals,
    blockers: [...blockers.values()].filter((entry) => entry.old !== entry.new).sort((a, b) => Math.abs(b.new - b.old) - Math.abs(a.new - a.old) || (a.kind < b.kind ? -1 : 1)),
    moves: [...moves.values()].map(({ projects: set, ...rest }) => ({ ...rest, projects: set.size })).sort(byTotal),
    needsLook: [...new Set(needsLook)].sort(),
  };
}

const pad = (value, width) => String(value).padStart(width);
const delta = (from, to) => (to === from ? "0" : to > from ? `+${to - from}` : `${to - from}`);

/** The report as text. `header` is the lines about the builds and the scope. */
export function render(report, header = []) {
  const out = [...header, ""];
  out.push(`projects run with the new build: ${report.projects.length}`, `  ${pad("V old", 6)} ${pad("V new", 6)} ${pad("U old", 7)} ${pad("U new", 7)} ${pad("U delta", 8)} ${pad("moved", 6)}  project (why)`);
  for (const project of report.projects) {
    out.push(`  ${pad(project.old.violation, 6)} ${pad(project.new.violation, 6)} ${pad(project.old.uncertifiable, 7)} ${pad(project.new.uncertifiable, 7)} ${pad(delta(project.old.uncertifiable, project.new.uncertifiable), 8)} ${pad(project.moved, 6)}  ${project.app}/${project.project} (${project.why.length > 70 ? `${project.why.slice(0, 67)}...` : project.why})`);
  }
  out.push(`  ${pad(report.totals.old.violation, 6)} ${pad(report.totals.new.violation, 6)} ${pad(report.totals.old.uncertifiable, 7)} ${pad(report.totals.new.uncertifiable, 7)} ${pad(delta(report.totals.old.uncertifiable, report.totals.new.uncertifiable), 8)}  total`);
  out.push("", "uncertifiable rows by blocker kind (only the kinds that moved)", `  ${pad("old", 7)} ${pad("new", 7)} ${pad("delta", 7)}  kind`);
  for (const entry of report.blockers) out.push(`  ${pad(entry.old, 7)} ${pad(entry.new, 7)} ${pad(delta(entry.old, entry.new), 7)}  ${entry.kind}`);
  if (report.blockers.length === 0) out.push("  none moved");
  out.push("", "moves by rule, kind and blocker", `  ${pad("rows", 7)} ${pad("projects", 8)}  rule  from -> to  (blocker)`);
  for (const move of report.moves) out.push(`  ${pad(move.rows, 7)} ${pad(move.projects, 8)}  ${move.rule}  ${move.from} -> ${move.to}  (${move.blocker})`);
  if (report.moves.length === 0) out.push("  none");
  out.push("", report.needsLook.length === 0 ? "needs a look: nothing" : `needs a look (${report.needsLook.length}):`);
  for (const line of report.needsLook) out.push(`  ${line}`);
  return `${out.join("\n")}\n`;
}

// ---------------------------------------------------------------------------------------------------------------------
// The run

/** The options of an argument list; throws on one it does not know or on a scope not named. */
export function parseOptions(argv) {
  const options = {
    old: null, new: null, typefacts: join(repo, "bin/solid-typefacts"), "old-typefacts": null, "new-typefacts": null,
    hazard: null, blocker: null, baseline: null, controls: "3",
    manifest: join(repo, "scripts/corpus/rc21-manifest.json"), "apps-root": resolve(repo, "../solid-checker-rc21-frozen/apps"),
    apps: null, jobs: "4", timeout: "900", "program-boundary": null, out: null,
  };
  const flags = new Set(["all", "json"]);
  const seen = new Set();
  for (let index = 0; index < argv.length; index += 1) {
    const key = argv[index].replace(/^--/, "");
    if (flags.has(key)) { seen.add(key); continue; }
    if (!(key in options) || argv[index + 1] === undefined) throw new Error(`unknown or incomplete option ${argv[index]}`);
    options[key] = argv[(index += 1)];
  }
  for (const name of ["old", "new"]) if (!options[name]) throw new Error(`--${name} <checker> is required`);
  const spec = {
    hazards: options.hazard ? options.hazard.split(",").filter(Boolean) : [],
    blockers: options.blocker ? options.blocker.split(",").filter(Boolean) : [],
    all: seen.has("all"),
  };
  if (!spec.all && spec.hazards.length === 0 && spec.blockers.length === 0) {
    throw new Error("name what the change reads: --hazard <id,...>, --blocker <kind,...>, or --all (a change that reads no recorded hazard moves rows anywhere)");
  }
  if (spec.all && (spec.hazards.length > 0 || spec.blockers.length > 0)) throw new Error("--all is the whole corpus: it takes no --hazard or --blocker");
  const controls = Number(options.controls);
  if (!Number.isInteger(controls) || controls < 0) throw new Error("--controls takes a count");
  return { options, spec, controls, json: seen.has("json") };
}

const sha256 = (path) => createHash("sha256").update(readFileSync(path)).digest("hex");

function runProject(checker, typefacts, row, configuration, boundary, timeoutMs) {
  const args = [
    "--format", "json", "--project", join(row.root, row.project),
    "--runtime-target", configuration.runtimeTarget, "--runtime-build", configuration.runtimeBuild, "--rendering", configuration.rendering,
    ...(boundary ? ["--program-boundary", boundary] : []),
  ];
  return new Promise((done) => {
    const child = spawn(checker, args, { cwd: row.root, env: { ...process.env, SOLID_CHECKER_DAEMON: "0", SOLID_TYPEFACTS_BIN: typefacts } });
    const stdout = [];
    const stderr = [];
    child.stdout.on("data", (chunk) => stdout.push(chunk));
    child.stderr.on("data", (chunk) => stderr.push(chunk));
    const timer = setTimeout(() => child.kill("SIGKILL"), timeoutMs);
    child.on("close", (exitCode, signal) => {
      clearTimeout(timer);
      let payload = null;
      let parseError = null;
      try { payload = JSON.parse(Buffer.concat(stdout).toString("utf8")); } catch (error) { parseError = `${error.name}: ${error.message}`; }
      done({ app: row.app, project: row.project, exitCode: signal ? null : exitCode, parseError, findings: payload?.findings ?? [], ...(payload && "hazards" in payload ? { hazards: payload.hazards } : {}), stderr: exitCode > 1 || parseError ? Buffer.concat(stderr).toString("utf8").slice(-600) : "" });
    });
  });
}

/** Runs `job(row)` for every row, `jobs` at a time, in the rows' order. */
async function pool(rows, jobs, job, label) {
  const results = new Array(rows.length);
  let next = 0;
  let done = 0;
  await Promise.all(Array.from({ length: jobs }, async () => {
    while (next < rows.length) {
      const index = next++;
      results[index] = await job(rows[index]);
      done += 1;
      process.stderr.write(`${label} ${done}/${rows.length} ${rows[index].app}/${rows[index].project}\n`);
    }
  }));
  return results;
}

async function main(argv) {
  const { options, spec, controls: controlCount, json } = parseOptions(argv);
  const manifestPath = resolve(options.manifest);
  const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  assertManifestProvenance(manifest, manifestPath);
  const appsRoot = resolve(options["apps-root"]);
  const selected = options.apps ? new Set(options.apps.split(",")) : null;
  const rows = manifest.projects.filter((row) => !selected || selected.has(row.app)).map((row) => ({ app: row.app, root: join(appsRoot, row.root), project: row.project }));
  for (const row of rows) if (!existsSync(join(row.root, row.project))) throw new Error(`missing project input: ${join(row.root, row.project)}`);
  const [oldBin, newBin] = [resolve(options.old), resolve(options.new)];
  const oldFacts = resolve(options["old-typefacts"] ?? options.typefacts);
  const newFacts = resolve(options["new-typefacts"] ?? options.typefacts);
  const timeoutMs = Number(options.timeout) * 1000;
  const jobs = Number(options.jobs);
  const boundary = options["program-boundary"];

  // The old build's results: the baseline report's, and a run for each project it lacks.
  const oldResults = new Map();
  if (options.baseline) {
    for (const result of JSON.parse(readFileSync(resolve(options.baseline), "utf8")).results) oldResults.set(keyOf(result), result);
  }
  const missing = rows.filter((row) => !oldResults.has(keyOf(row)));
  for (const result of await pool(missing, jobs, (row) => runProject(oldBin, oldFacts, row, manifest.configuration, boundary, timeoutMs), "old")) oldResults.set(keyOf(result), result);
  const inScope = new Map(rows.map((row) => [keyOf(row), oldResults.get(keyOf(row))]));
  const { affected, unaffected } = scope(inScope, spec);
  const controls = pickControls(inScope, unaffected, controlCount);
  const toRun = rows.filter((row) => affected.has(keyOf(row)) || controls.includes(keyOf(row)));
  const newResults = new Map();
  for (const result of await pool(toRun, jobs, (row) => runProject(newBin, newFacts, row, manifest.configuration, boundary, timeoutMs), "new")) newResults.set(keyOf(result), result);

  const errors = [];
  for (const [side, results] of [["old", oldResults], ["new", newResults]]) {
    for (const [key, result] of results) {
      if (side === "old" && !newResults.has(key)) continue;
      if (result.parseError) errors.push(`${side} build, ${key.replace("\u0000", "/")}: ${result.parseError} ${result.stderr ?? ""}`.trim());
      else if (result.exitCode === null) errors.push(`${side} build, ${key.replace("\u0000", "/")}: killed (timeout)`);
      else if (result.exitCode > 1) errors.push(`${side} build, ${key.replace("\u0000", "/")}: exit ${result.exitCode} ${result.stderr ?? ""}`.trim());
    }
  }
  const report = buildReport({ oldResults: new Map([...oldResults].filter(([key]) => newResults.has(key))), newResults, affected, controls, errors });
  const header = [
    `corpus delta: old ${oldBin} (sha256 ${sha256(oldBin).slice(0, 12)}), new ${newBin} (sha256 ${sha256(newBin).slice(0, 12)})`,
    `corpus ${manifest.corpus ?? manifestPath}: ${rows.length} projects; scope ${spec.all ? "all" : [spec.hazards.length ? `hazards ${spec.hazards.join(", ")}` : null, spec.blockers.length ? `blockers ${spec.blockers.join(", ")}` : null].filter(Boolean).join(" or ")}: ${affected.size} affected, ${controls.length} controls, ${unaffected.length - controls.length} not run`,
    ...(options.baseline ? [`old results: ${options.baseline}${missing.length ? ` and ${missing.length} projects run here` : ""}`] : ["old results: run here"]),
  ];
  if (options.out) writeFileSync(resolve(options.out), `${JSON.stringify({ header, ...report }, null, 2)}\n`);
  process.stdout.write(json ? `${JSON.stringify({ header, ...report }, null, 2)}\n` : render(report, header));
  process.exitCode = report.needsLook.length > 0 ? 1 : 0;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) await main(process.argv.slice(2));
