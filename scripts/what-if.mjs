#!/usr/bin/env node
// ADR 0950: what-if runs and their predictions. `solid-checker-what-if --assume <hazards>` (`make build-what-if`) analyzes a project with the named hazard facts treated as
// absent; this script runs it next to the shipped checker (a default build of the same source) and reports what the assumption would move: per site, the proof-ablation harness's
// transitions (scripts/proof-ablation.mjs `transitions`, `summarize`: an obligation that becomes a violation `uToV`, one that disappears `uLost`, a violation that is lost `vLost`,
// new ones `cleanToU`/`cleanToV`), and per uncertifiable row the group it stood in, `(kind, formula, fired set)` (ADR 0954), so the numbers say which rows each way of lifting a veto
// (`liftingSets`, scripts/u-blockers.mjs) would move and where.
//
// A prediction is no measurement of the shipped checker: the report is `kind: "what-if"` and says so, and every other consumer of a checker report refuses a what-if one
// (scripts/lib/assumed.mjs). THIS file is the one reader of such a report, with a reader of its own (`readWhatIf`: it wants the taint exactly as the build writes it, and refuses
// a report without it, so a default build handed in as the what-if binary is an error, not a baseline). It compares only runs it makes itself, with the two binaries it is given.
//
//   node scripts/what-if.mjs corpus --what-if-bin <solid-checker-what-if> --checker-bin <solid-checker-rust, a default build of the same source> --typefacts <solid-typefacts>
//        --set name=Hazard,Hazard ... [--sets-file f.json] [--sets-from-baseline N] [--premise-sets N] [--manifest scripts/corpus/rc21-manifest.json] [--apps-root ../solid-checker-rc21-frozen/apps] [--apps a,b]
//        [--order-from sweep.json] [--jobs 12] [--no-control] --out report.json
//   node scripts/what-if.mjs grid --what-if-bin ... --checker-bin ... --typefacts ... --scan <blocker-scan rows.jsonl> --set name=ids ... [--sets-file f] [--jobs 12]
//        [--programs-from all] [--control N] [--every K] [--limit N] --out report.json [--movers-out ids.json]
//        ADR 0965: a `Premise(<kind>)` set runs the scan's programs that carry a row of that cause (the scan's `u.kinds`), not only the ones with a formula row.
//   node scripts/what-if.mjs rank --report corpus.json [--judged judged.json] [--top 40]   ADR 0965: the sets of a corpus report ranked by the rows and sites they lift, with what Chrome said of
//        the same sets on the grids (`judge`'s report) beside each
//   node scripts/what-if.mjs sets --scan <blocker-scan rows.jsonl> [--top 30]      the lifting sets of the scan's formula groups, with their rows and programs
//   node scripts/what-if.mjs movers --report grid.json --out <manifest dir> [--run-out dir] [--checker-bin ... --typefacts ...]   the programs that moved, as the commands that run them in Chrome
//   node scripts/what-if.mjs judge --report grid.json --chrome <dir of chrome-*.json> --out judged.json    each moved row against Chrome's record: false clean, false positive, confirmed
//
// `corpus` runs every project of the manifest once with the shipped checker and once per set with the what-if build, and, unless `--no-control`, once more with the what-if build
// assuming nothing: that report must be the shipped checker's, key for key but for the taint (a control that differs stops the run, since every number after it would stand on
// a build that is not the one asked about). `grid` regenerates the programs of a blocker scan (ADR 0954) and runs the programs whose rows fire a hazard of a set under it (`--programs-from
// all`: every program with a formula row, under every set); the control runs on `--control N` programs spread over the selection (200 by default, `0` for none).
import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { appendFileSync, createReadStream, existsSync, mkdirSync, readFileSync, readdirSync, realpathSync, rmSync, statSync, writeFileSync } from "node:fs";
import { basename, dirname, join, resolve } from "node:path";
import { createInterface } from "node:readline";
import process from "node:process";
import { fileURLToPath } from "node:url";

import { codesForRule, codeRules, ruleHasRuntimeCounterpart } from "./fuzz/codes.mjs";
import { refuseAssumed } from "./lib/assumed.mjs";
import { findingSites, summarize, transitions } from "./proof-ablation.mjs";
import { liftingSets } from "./u-blockers.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const repo = resolve(here, "..");

/** The structure a `--set` names: `{ name, ids }`, ids as the build reads them (comma separated, deduplicated in order). */
export function parseSet(text) {
  const at = text.indexOf("=");
  if (at <= 0) throw new Error(`--set takes name=Hazard,Hazard (got ${text})`);
  const ids = [...new Set(text.slice(at + 1).split(",").filter(Boolean))];
  return { name: text.slice(0, at), ids };
}

/**
 * The shipped checker's report of a run: parsed, and refused when it is tainted (a what-if build handed in as the shipped checker would make every baseline a prediction).
 * @param {string} text what the checker printed
 */
export function readBaseline(text) {
  return refuseAssumed(JSON.parse(text), "scripts/what-if.mjs (the baseline checker)");
}

/**
 * The what-if build's report of a run, read as only this script reads one: it must carry the taint exactly as the build writes it, for the assumption it was given:
 * `assumed.hazards` the ids as given (deduplicated, in order), `assumed.flipped` a subset of them, and when something flipped an `assumed` premise on every finding naming exactly
 * the flipped ids, none when nothing did. A report without it is not a what-if report, and nothing here compares one.
 * @param {string} text what the build printed
 * @param {string[]} ids the assumption the run was given
 */
export function readWhatIf(text, ids) {
  const report = JSON.parse(text);
  const fail = (why) => { throw new Error(`not the what-if build's report: ${why}`); };
  if (report === null || typeof report !== "object" || Array.isArray(report)) fail("not an object");
  const { assumed } = report;
  if (assumed === null || typeof assumed !== "object") fail("no `assumed`: a default build was handed in as the what-if binary");
  const expected = [...new Set(ids)];
  if (JSON.stringify(assumed.hazards) !== JSON.stringify(expected)) fail(`\`assumed.hazards\` is ${JSON.stringify(assumed.hazards)}, the run asked for ${JSON.stringify(expected)}`);
  if (!Array.isArray(assumed.flipped) || assumed.flipped.some((id) => !expected.includes(id))) fail(`\`assumed.flipped\` ${JSON.stringify(assumed.flipped)} is not a subset of the assumption`);
  for (const finding of report.findings ?? []) {
    const premises = (finding.premises ?? []).filter((premise) => premise?.kind === "assumed");
    if (assumed.flipped.length === 0 ? premises.length !== 0 : premises.length !== 1 || JSON.stringify(premises[0].hazards) !== JSON.stringify(assumed.flipped)) fail(`a finding's \`assumed\` premises ${JSON.stringify(premises)} do not name the flipped hazards ${JSON.stringify(assumed.flipped)}`);
  }
  return report;
}

/** A value with its object keys sorted at every depth: two reports are the same report when this is. */
export function canonical(value) {
  if (Array.isArray(value)) return value.map(canonical);
  if (value && typeof value === "object") return Object.fromEntries(Object.keys(value).sort().map((key) => [key, canonical(value[key])]));
  return value;
}

/** The what-if report without its taint, as the shipped checker writes it: no `assumed`, no `premises` on a finding. */
export function untainted(report) {
  const { assumed: _assumed, ...rest } = report;
  return { ...rest, findings: (report.findings ?? []).map(({ premises: _premises, ...finding }) => finding) };
}

/** Whether a what-if run that assumed nothing is the shipped checker's report (the control). */
export const sameReport = (shipped, whatIf) => JSON.stringify(canonical(shipped)) === JSON.stringify(canonical(untainted(whatIf)));

const spanOf = (finding) => `${finding.primaryLocation?.path ?? ""}:${finding.primaryLocation?.startByte ?? 0}:${finding.primaryLocation?.endByte ?? 0}`;

/** The group an uncertifiable row stands in (ADR 0954): `kind formula [fired]`, or the kind alone for a row whose blocker names no formula. */
export function groupOf(finding) {
  const blocked = finding.blockedBy;
  if (!blocked) return "no-blocker";
  return blocked.formula ? `${blocked.kind} ${blocked.formula} [${[...(blocked.fired ?? [])].sort().join(", ")}]` : blocked.kind;
}

/** A finding as the Chrome comparison reads it: its file (relative to the program's directory `root` when under it), line, code, kind and rule. */
const compact = (finding, root) => {
  const path = finding.primaryLocation?.path ?? "";
  return { file: root && path.startsWith(`${root}/`) ? path.slice(root.length + 1) : path, line: finding.primaryLocation?.line ?? 0, id: finding.id, kind: finding.kind, rule: finding.rule };
};

/**
 * What became of each uncertifiable row of the shipped checker's report in the what-if report: matched by its span. `toV` the same code is now a violation at the span, `toVOther`
 * another code is; `stillU` the same code is still an obligation there with the same blocker, `blockedChanged` with another (the veto lifted, another cause stands), `toUOther` another
 * code is an obligation there; `besideV` the obligation is gone and the only findings left are violations the shipped report held at the span already; `toClean` nothing is left at the
 * span. A violation counts as a move only when the shipped report had no violation of that code at the span: a site that was a violation stays one (as `transitions` reads a site by its
 * kind), so an obligation beside a violation of the shipped checker is not a `toV` row, whatever the assumption. Findings of the what-if report at spans the shipped report has none at are
 * `new`. Returns `{ rows: [{ group, outcome, file, line, id, kind, rule, now, fresh }], new: [{ file, line, id, kind, rule }] }`, `now` the findings at the row's span in the what-if
 * report and `fresh` the violations among them the shipped report did not hold there (what a verdict on the row is about); `root` is the program's directory.
 */
export function rowOutcomes(base, whatIf, root = "") {
  const at = new Map();
  for (const finding of whatIf.findings ?? []) {
    if (/\/main\.tsx$/.test(finding.primaryLocation?.path ?? "")) continue;
    const span = spanOf(finding);
    at.set(span, [...(at.get(span) ?? []), finding]);
  }
  const held = new Map();
  for (const finding of base.findings ?? []) {
    if (/\/main\.tsx$/.test(finding.primaryLocation?.path ?? "") || finding.kind !== "violation") continue;
    const span = spanOf(finding);
    held.set(span, new Set([...(held.get(span) ?? []), finding.id]));
  }
  const rows = [];
  const baseSpans = new Set();
  for (const finding of base.findings ?? []) {
    if (/\/main\.tsx$/.test(finding.primaryLocation?.path ?? "")) continue;
    const span = spanOf(finding);
    baseSpans.add(span);
    if (finding.kind !== "uncertifiable") continue;
    const here = at.get(span) ?? [];
    const fresh = here.filter((candidate) => candidate.kind === "violation" && !held.get(span)?.has(candidate.id));
    const same = here.filter((candidate) => candidate.id === finding.id && candidate.kind !== "violation");
    let outcome;
    if (fresh.some((candidate) => candidate.id === finding.id)) outcome = "toV";
    else if (fresh.length) outcome = "toVOther";
    else if (same.length) outcome = same.some((candidate) => groupOf(candidate) === groupOf(finding)) ? "stillU" : "blockedChanged";
    else if (here.some((candidate) => candidate.kind !== "violation")) outcome = "toUOther";
    else if (here.length) outcome = "besideV";
    else outcome = "toClean";
    rows.push({ group: groupOf(finding), outcome, ...compact(finding, root), now: here.map((candidate) => compact(candidate, root)), fresh: fresh.map((candidate) => compact(candidate, root)) });
  }
  const added = [];
  for (const [span, findings] of at) if (!baseSpans.has(span)) added.push(...findings.map((finding) => compact(finding, root)));
  return { rows, new: added };
}

/** The outcomes that make a row a U→V and a U→clean prediction. */
export const TO_V = new Set(["toV", "toVOther"]);

/** Adds `outcomes` (a program's `rowOutcomes`) to a group tally: `{ rows, toV, toVOther, stillU, blockedChanged, toUOther, besideV, toClean }`. */
export function addOutcomes(tally, outcomes, key = (row) => row.group) {
  for (const row of outcomes.rows) {
    const entry = (tally[key(row)] ??= { rows: 0, toV: 0, toVOther: 0, stillU: 0, blockedChanged: 0, toUOther: 0, besideV: 0, toClean: 0 });
    entry.rows += 1;
    entry[row.outcome] += 1;
  }
  return tally;
}

// --- runs of the two binaries ---------------------------------------------------------------------------------------------------------------------------------------

const running = new Set();

/** One run of a checker: `{ status, stdout, stderr, wallMs }`. */
function run(bin, args, { cwd, env, timeoutMs = 900_000 }) {
  return new Promise((done) => {
    const started = performance.now();
    const child = spawn(bin, args, { cwd, env });
    running.add(child);
    const out = [];
    const err = [];
    child.stdout.on("data", (chunk) => out.push(chunk));
    child.stderr.on("data", (chunk) => err.push(chunk));
    const timer = setTimeout(() => child.kill("SIGKILL"), timeoutMs);
    child.on("error", (error) => { clearTimeout(timer); running.delete(child); done({ status: null, stdout: "", stderr: String(error), wallMs: 0 }); });
    child.on("close", (status, signal) => {
      clearTimeout(timer);
      running.delete(child);
      done({ status: signal ? null : status, stdout: Buffer.concat(out).toString("utf8"), stderr: Buffer.concat(err).toString("utf8"), wallMs: Math.round(performance.now() - started) });
    });
  });
}

const checkerEnvironment = (typefacts) => ({ ...process.env, SOLID_CHECKER_DAEMON: "0", SOLID_TYPEFACTS_BIN: typefacts });
const sha256 = (path) => createHash("sha256").update(readFileSync(path)).digest("hex");

/**
 * Runs `tasks` with at most `jobs` at a time; a rejection stops the pool where it stands (no new task starts, the checkers in flight are killed) and rejects with it.
 * @template T
 * @param {(() => Promise<T>)[]} tasks
 */
async function pool(tasks, jobs) {
  const results = new Array(tasks.length);
  let next = 0;
  let failure = null;
  const worker = async () => {
    while (next < tasks.length && !failure) {
      const at = next++;
      try {
        results[at] = await tasks[at]();
      } catch (error) {
        failure ??= error;
        for (const child of running) child.kill("SIGKILL");
      }
    }
  };
  await Promise.all(Array.from({ length: jobs }, worker));
  if (failure) throw failure;
  return results;
}

/** The shipped checker's run of a program: `{ baseline, summary }`, or `{ error }`. */
async function baselineRun({ program, binaries, env = checkerEnvironment(binaries.typefacts) }) {
  const base = await run(binaries.checker, program.args, { cwd: program.cwd, env });
  if (base.status !== 0 && base.status !== 1) return { error: `the shipped checker exited ${base.status}: ${base.stderr.slice(-300)}` };
  const baseline = readBaseline(base.stdout);
  const findings = baseline.findings ?? [];
  return { baseline, summary: { status: baseline.status, rows: { u: findings.filter((finding) => finding.kind === "uncertifiable").length, v: findings.filter((finding) => finding.kind === "violation").length }, hazards: (baseline.hazards ?? []).map((hazard) => hazard.id), wallMs: base.wallMs } };
}

/** The what-if build's run of a program under the assumption `ids`, set against the shipped checker's `baseline`: the entry of the program's `bySet`. */
async function whatIfRun({ program, baseline, name, ids, binaries, env = checkerEnvironment(binaries.typefacts), root = "" }) {
  const answer = await run(binaries.whatIf, [...program.args, "--assume", ids.join(",")], { cwd: program.cwd, env });
  if (answer.status !== 0 && answer.status !== 1) {
    const error = `the what-if build ${answer.status === null ? "was killed (its time limit) or did not start" : `exited ${answer.status}`}: ${answer.stderr.slice(-300)}`;
    // The control is what makes every other number of a program the shipped checker's: a control that fails stops the run and names itself (it is not counted and passed over).
    if (name === "control") throw new Error(`the control run failed at ${program.id}: ${error}`);
    return { error };
  }
  const report = readWhatIf(answer.stdout, ids);
  if (name === "control") {
    if (!sameReport(baseline, report)) throw new Error(`the control differs from the shipped checker at ${program.id}: a what-if run that assumed nothing must report what the shipped checker reports`);
    return { same: true, flipped: report.assumed.flipped };
  }
  return { flipped: report.assumed.flipped, transitions: transitions(findingSites(baseline), findingSites(report)), outcomes: rowOutcomes(baseline, report, root), status: report.status, now: (report.findings ?? []).filter((finding) => !/\/main\.tsx$/.test(finding.primaryLocation?.path ?? "")).map((finding) => compact(finding, root)) };
}

/** What a stored result keeps of a run: what moved (rows that did not stay as they were, findings that are new, the whole list of findings now), not every row. */
export function slim(entry, { keepNow = true } = {}) {
  if (!entry.transitions) return entry;
  const moves = Object.values(entry.transitions).some((list) => list.length > 0);
  const counts = {};
  for (const row of entry.outcomes.rows) counts[row.outcome] = (counts[row.outcome] ?? 0) + 1;
  // `keepNow: false` (the corpus, whose rows no Chrome record judges) leaves out the findings now and the new ones: the moved rows and the counts are what a project's report keeps.
  return { flipped: entry.flipped, status: entry.status, rows: counts, ...(moves ? { transitions: entry.transitions, moved: entry.outcomes.rows.filter((row) => row.outcome !== "stillU").map(({ now, ...row }) => (keepNow ? { ...row, now } : row)), ...(keepNow ? { new: entry.outcomes.new, now: entry.now } : {}) } : {}) };
}

/** One program under the shipped checker and under each of `sets` with the what-if build, one after the other; the program's result. */
async function measure({ program, sets, binaries, control, env, root = "" }) {
  const base = await baselineRun({ program, binaries, env });
  if (base.error) return { program: program.id, error: base.error };
  const bySet = {};
  for (const { name, ids } of control ? [{ name: "control", ids: [] }, ...sets] : sets) bySet[name] = await whatIfRun({ program, baseline: base.baseline, name, ids, binaries, env, root });
  return { program: program.id, ...base.summary, bySet };
}

/** A program's result as stored: each set's entry slimmed. */
export const slimResult = ({ bySet, ...rest }) => ({ ...rest, ...(bySet ? { bySet: Object.fromEntries(Object.entries(bySet).map(([name, entry]) => [name, slim(entry)])) } : {}) });

const NO_MOVES = { cleanToU: [], cleanToV: [], vToU: [], vLost: [], uToV: [], uLost: [] };

/** The state a fold starts from: one tally per set for the harness, the groups, the projects and the programs that moved, and the control's. */
export function newFold(sets) {
  const byName = (make) => Object.fromEntries(sets.map(({ name }) => [name, make()]));
  return { sets, harness: [], groups: byName(() => ({})), projects: byName(() => ({})), moved: byName(() => []), control: { programs: 0, same: 0, flipped: 0 }, errors: 0 };
}

/** Adds one program's result (with its full entries) to a fold state: the harness's transitions per set, group tallies per set and per project. */
export function foldOne(state, result) {
  if (result.error) { state.errors += 1; return; }
  if (result.bySet.control) {
    const control = result.bySet.control;
    if (control.error) throw new Error(`the control run failed at ${result.program}: ${control.error}`);
    state.control.programs += 1;
    state.control.same += control.same ? 1 : 0;
    state.control.flipped += control.flipped.length;
  }
  for (const { name } of state.sets) {
    const entry = result.bySet[name];
    if (!entry || entry.error) { if (entry?.error) state.errors += 1; continue; }
    const changed = Object.values(entry.transitions).some((list) => list.length > 0);
    state.harness.push({ fixture: result.program, arm: name, changed, transitions: changed ? entry.transitions : NO_MOVES });
    addOutcomes(state.groups[name], entry.outcomes);
    const project = (state.projects[name][result.project ?? result.program] ??= { programs: 1, rows: 0, toV: 0, toClean: 0, stillU: 0, other: 0, besideV: 0, newV: 0, newU: 0 });
    for (const row of entry.outcomes.rows) {
      project.rows += 1;
      if (TO_V.has(row.outcome)) project.toV += 1;
      else if (row.outcome === "toClean") project.toClean += 1;
      else if (row.outcome === "stillU") project.stillU += 1;
      else if (row.outcome === "besideV") project.besideV += 1;
      else project.other += 1;
    }
    project.newV += entry.outcomes.new.filter((one) => one.kind === "violation").length;
    project.newU += entry.outcomes.new.filter((one) => one.kind !== "violation").length;
    if (changed) state.moved[name].push(result.program);
  }
}

/** The fold's report: the harness's `summarize` per set (a count of the programs a set did not move, not their ids, which made a first corpus report too long to write), groups, projects. */
export function finishFold(state) {
  const { arms, unwitnessed } = summarize(state.harness);
  for (const arm of Object.values(arms)) arm.inert = arm.inert.length;
  return { arms, unwitnessed, groups: state.groups, projects: state.projects, moved: state.moved, control: state.control, errors: state.errors };
}

/** Folds the programs' results into the report: the harness's `summarize` per set, group tallies per set and per project. */
export function fold(results, sets) {
  const state = newFold(sets);
  for (const result of results) foldOne(state, result);
  return finishFold(state);
}

/** The `--flag value` options of an argument list; `--set` repeats. */
function options(argv, known) {
  const found = { set: [] };
  for (let at = 0; at < argv.length; at += 1) {
    const key = argv[at].replace(/^--/, "");
    if (!argv[at].startsWith("--") || !known.includes(key)) throw new Error(`unknown option ${argv[at]}`);
    if (key === "no-control") { found[key] = true; continue; }
    if (argv[at + 1] === undefined) throw new Error(`${argv[at]} needs a value`);
    if (key === "set") found.set.push(argv[++at]);
    else found[key] = argv[++at];
  }
  return found;
}

function setsOf(found, { optional = false } = {}) {
  const sets = found.set.map(parseSet);
  if (found["sets-file"]) for (const [name, ids] of Object.entries(JSON.parse(readFileSync(resolve(found["sets-file"]), "utf8")))) sets.push({ name, ids: [...new Set(ids)] });
  if (sets.length === 0 && !optional) throw new Error("name at least one assumption: --set name=Hazard,Hazard or --sets-file");
  if (new Set(sets.map(({ name }) => name)).size !== sets.length || sets.some(({ name }) => name === "control")) throw new Error("set names are distinct, and `control` is the run that assumes nothing");
  return sets;
}

function binariesOf(found) {
  const binaries = { whatIf: resolve(found["what-if-bin"] ?? ""), checker: resolve(found["checker-bin"] ?? ""), typefacts: resolve(found.typefacts ?? "") };
  for (const [name, path] of Object.entries(binaries)) if (!existsSync(path)) throw new Error(`--${name === "whatIf" ? "what-if-bin" : name === "checker" ? "checker-bin" : "typefacts"} ${path} does not exist`);
  if (binaries.whatIf === binaries.checker) throw new Error("the what-if build and the shipped checker are two binaries");
  return binaries;
}

const header = (binaries, extra) => ({
  kind: "what-if",
  note: "A prediction: the what-if build treats the named hazard facts as absent. Counts of programs are counts only, no safety; scripts/lib/assumed.mjs makes every other consumer refuse these runs.",
  whatIf: { path: binaries.whatIf, sha256: sha256(binaries.whatIf) },
  checker: { path: binaries.checker, sha256: sha256(binaries.checker) },
  producer: { path: binaries.typefacts, sha256: sha256(binaries.typefacts) },
  ...extra,
});

/**
 * ADR 0965: one set per typed cause (`blockedBy.kind`) that at least `minRows` uncertifiable rows of the shipped checker's reports stand under: `Premise(<kind>)`, the cause assumed away
 * as a whole (every row of that kind leaves the report, whatever the veto or placement behind it). `existing` is not asked twice. The biggest cause first, then by name.
 */
export function premiseSetsFromBaselines(baselines, existing, minRows = 20) {
  const kinds = new Map();
  for (const baseline of baselines) {
    for (const finding of baseline.findings ?? []) {
      const kind = finding.blockedBy?.kind;
      if (finding.kind !== "uncertifiable" || typeof kind !== "string" || !/^[a-z0-9]+(?:-[a-z0-9]+)+$/.test(kind)) continue;
      kinds.set(kind, (kinds.get(kind) ?? 0) + 1);
    }
  }
  const have = new Set(existing.map(({ ids }) => [...ids].sort().join("\0")));
  const derived = [...kinds]
    .filter(([kind, rows]) => rows >= minRows && !have.has(`Premise(${kind})`))
    .sort((a, b) => b[1] - a[1] || (a[0] < b[0] ? -1 : 1))
    .map(([kind]) => ({ name: `premise:${kind}`, ids: [`Premise(${kind})`] }));
  // The causes only the what-if build tells apart (PREMISE_PARENT) are asked beside the kind they share, and with it: the baselines cannot count them (the shipped checker's rows hold the shared kind).
  const variants = Object.entries(PREMISE_PARENT)
    .filter(([variant, parent]) => (kinds.get(parent) ?? 0) >= minRows && !have.has(`Premise(${variant})`))
    .map(([variant]) => ({ name: `premise:${variant}`, ids: [`Premise(${variant})`] }));
  return [...derived, ...variants];
}

/** The hazard ids the what-if build can name (ADR 0947's atoms): a lifting set with another atom (the array-slots formula's) is not asked of it. */
const KNOWN_ATOM = /^(OpenMemberWrite\(.+\)|StandardImpostor(Write)?\(.+\)|StandardOverride\(.+\)|StandardPatch\(.+\)|ReflectiveDefinitionSpelling|ReflectiveDefinition\(.+\)|StandardRoot(Handed|Received|Iterated)|RootPatch|AnyStandardPatch)$/;

/**
 * The lifting sets (`liftingSets`) of every veto group the shipped checker's reports of the corpus hold, that the build can name, that at least `minRows` rows stand in and that `existing`
 * does not already ask: `[{ name, ids }]`, the biggest first. The corpus asks for its own groups' ways to lift, not only the grids' (a project's groups have atoms the grids' do not).
 */
export function setsFromBaselines(baselines, existing, minRows = 20) {
  const groups = new Map();
  for (const baseline of baselines) {
    for (const finding of baseline.findings ?? []) {
      const blocked = finding.blockedBy;
      if (finding.kind !== "uncertifiable" || !blocked?.formula) continue;
      const key = JSON.stringify([blocked.formula, [...(blocked.fired ?? [])].sort()]);
      groups.set(key, (groups.get(key) ?? 0) + 1);
    }
  }
  const found = new Map();
  for (const [key, rows] of groups) {
    const [formula, fired] = JSON.parse(key);
    for (const set of liftingSets(formula, fired)) {
      if (!set.every((id) => KNOWN_ATOM.test(id))) continue;
      const name = set.join(" + ");
      found.set(name, { name, ids: set, rows: (found.get(name)?.rows ?? 0) + rows });
    }
  }
  const have = new Set(existing.map(({ ids }) => [...ids].sort().join("\0")));
  return [...found.values()]
    .filter((set) => set.rows >= minRows && !have.has([...set.ids].sort().join("\0")))
    .sort((a, b) => b.rows - a.rows || (a.name < b.name ? -1 : 1))
    .map(({ name, ids }) => ({ name, ids }));
}

async function corpus(argv) {
  const found = options(argv, ["what-if-bin", "checker-bin", "typefacts", "set", "sets-file", "sets-from-baseline", "premise-sets", "manifest", "apps-root", "apps", "order-from", "jobs", "no-control", "out"]);
  if (!found.out) throw new Error("--out <report.json> is required");
  let sets = setsOf(found, { optional: Boolean(found["sets-from-baseline"] || found["premise-sets"]) });
  const binaries = binariesOf(found);
  const manifestPath = resolve(found.manifest ?? join(repo, "scripts/corpus/rc21-manifest.json"));
  const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  const appsRoot = resolve(found["apps-root"] ?? join(repo, "../solid-checker-rc21-frozen/apps"));
  const selected = found.apps ? new Set(found.apps.split(",")) : null;
  const rows = manifest.projects.filter((row) => !selected || selected.has(row.app)).map((row) => ({ app: row.app, root: join(appsRoot, row.root), project: row.project }));
  for (const row of rows) if (!existsSync(join(row.root, row.project))) throw new Error(`missing project input: ${join(row.root, row.project)}`);
  if (found["order-from"]) {
    const previous = new Map(JSON.parse(readFileSync(resolve(found["order-from"]), "utf8")).results.map((row) => [`${row.app}\0${row.project}`, row.wallMs]));
    rows.sort((left, right) => (previous.get(`${right.app}\0${right.project}`) ?? Number.MAX_SAFE_INTEGER) - (previous.get(`${left.app}\0${left.project}`) ?? Number.MAX_SAFE_INTEGER));
  }
  const configuration = manifest.configuration;
  const programs = rows.map((row) => ({ row, id: `${row.app}/${row.project}`, cwd: row.root, args: ["--format", "json", "--project", join(row.root, row.project), "--runtime-target", configuration.runtimeTarget, "--runtime-build", configuration.runtimeBuild, "--rendering", configuration.rendering] }));
  const jobs = Number(found.jobs ?? 4);
  // The shipped checker once per project, then every (project, set) run on its own: one slow project does not hold the others' sets behind it.
  const bases = await pool(programs.map((program) => () => baselineRun({ program, binaries })), jobs);
  process.stderr.write(`what-if corpus: ${programs.length} project(s) under the shipped checker\n`);
  // `--sets-from-baseline N`: besides the sets named, the lifting sets of the corpus's own veto groups with at least N rows.
  if (found["sets-from-baseline"]) {
    const derived = setsFromBaselines(bases.filter((base) => !base.error).map((base) => base.baseline), sets, Number(found["sets-from-baseline"]));
    sets = [...sets, ...derived];
    process.stderr.write(`what-if corpus: ${derived.length} lifting set(s) of the corpus's own groups added (${sets.length} in all)\n`);
  }
  // `--premise-sets N` (ADR 0965): one set per typed cause with at least N uncertifiable rows in the corpus's own reports.
  if (found["premise-sets"]) {
    const derived = premiseSetsFromBaselines(bases.filter((base) => !base.error).map((base) => base.baseline), sets, Number(found["premise-sets"]));
    sets = [...sets, ...derived];
    process.stderr.write(`what-if corpus: ${derived.length} typed cause(s) added as premise sets (${sets.length} in all)\n`);
  }
  const asked = [...(found["no-control"] ? [] : [{ name: "control", ids: [] }]), ...sets];
  // Every (project, set) run is folded into the report's tallies the moment it ends, and only its slim entry is kept: the full rows of 34 projects under every set (kui's have 4,000 each) are
  // what filled the parent's memory in the first runs. A project whose baseline failed is one error.
  const state = newFold(sets);
  programs.forEach((program, at) => { if (bases[at].error) foldOne(state, { program: program.id, error: bases[at].error }); });
  const jobsList = programs.flatMap((program, at) => (bases[at].error ? [] : asked.map(({ name, ids }) => ({ program, at, name, ids }))));
  const resultsFile = `${basename(resolve(found.out), ".json")}.results.jsonl`;
  const resultsPath = join(dirname(resolve(found.out)), resultsFile);
  mkdirSync(dirname(resolve(found.out)), { recursive: true });
  writeFileSync(resultsPath, "");
  await pool(jobsList.map(({ program, name, ids, at }) => async () => {
    const entry = await whatIfRun({ program, baseline: bases[at].baseline, name, ids, binaries });
    process.stderr.write(`${program.id} ${name}: ${entry.error ?? (entry.transitions ? `${entry.transitions.uToV.length} U->V, ${entry.transitions.uLost.length} U->clean` : "same")}\n`);
    foldOne(state, { program: program.id, project: program.id, bySet: { [name]: entry } });
    // One line a (project, set), written as it ends and not kept: a project's sets together are too long for one string.
    appendFileSync(resultsPath, `${JSON.stringify({ program: program.id, project: program.id, app: program.row.app, set: name, entry: slim(entry, { keepNow: false }) })}\n`);
    return null;
  }), jobs);
  const baselines = programs.map((program, at) => (bases[at].error ? { program: program.id, error: bases[at].error } : { program: program.id, ...bases[at].summary }));
  const report = { ...header(binaries, { source: "corpus", manifest: { path: manifestPath, sha256: sha256(manifestPath) }, configuration, sets, programs: programs.length }), ...finishFold(state), baselines, resultsFile };
  writeFileSync(resolve(found.out), `${JSON.stringify(report, null, 2)}\n`);
  process.stdout.write(summaryText(report, sets));
}

/**
 * The lines of a blocker scan (ADR 0954): the programs with formula rows, `{ id, grid, formulas: [{ kind, formula, fired }] }`. ADR 0965: with `kinds`, also the programs that carry an uncertifiable
 * row of some cause (`u.kinds`, the scan's count of every row by its `blockedBy.kind`), `{ ..., kinds: [<kind>] }`: what a `Premise(<kind>)` set can move.
 */
async function scanPrograms(path, { kinds = false } = {}) {
  const programs = [];
  for await (const line of createInterface({ input: createReadStream(resolve(path)) })) {
    if (line.startsWith('{"scan"')) continue;
    const row = JSON.parse(line);
    if (row.error) continue;
    const present = kinds ? Object.entries(row.u?.kinds ?? {}).filter(([, counts]) => counts.some((count) => count > 0)).map(([kind]) => kind) : [];
    if (!row.formulas?.length && !present.length) continue;
    programs.push({ id: row.id, grid: row.grid, code: row.code, context: row.context, side: row.side, formulas: row.formulas ?? [], kinds: present });
  }
  return programs;
}

/** The cause a `Premise(<kind>)` id names, or null for any other id. */
export const premiseOf = (id) => /^Premise\(([a-z0-9]+(?:-[a-z0-9]+)+)\)$/.exec(id)?.[1] ?? null;

/**
 * ADR 0965, what-if slice 2: the causes only the what-if build tells apart, and the blocker kind the shipped checker's rows (and so the scan's `u.kinds`) hold them under. The two climb stops
 * that owe an obligation (ADR 0959) are SC3001's uncertifiable `leaf-owner-unproven` rows in every report, the blocker every uncertain leaf operation shares; the what-if build knows which stop
 * each row is owed at, so `Premise(<stop>)` lifts the rows of that stop alone and `Premise(leaf-owner-unproven)` the rest. A program is selected for a variant by the kind it shares.
 */
export const PREMISE_PARENT = {
  "leaf-owner-climb-unmodelled-in-place": "leaf-owner-unproven",
  "leaf-owner-climb-bound": "leaf-owner-unproven",
};

/** The kind of the scan's `u.kinds` a `Premise(<kind>)` set selects its programs by. */
export const premiseSelectionKind = (kind) => PREMISE_PARENT[kind] ?? kind;

/**
 * Whether the grid's program can move under `set` (so it is run under it): one of its formula rows fires a hazard of the set (the seam answers only for a hazard the program has), or, for a
 * `Premise(<kind>)` id (ADR 0965), the program carries a row of that cause; `all` (`--programs-from all`) runs every program under every set.
 */
export function programWantsSet(program, set, all = false) {
  if (all) return true;
  return program.formulas.some(({ fired }) => fired.some((id) => set.ids.includes(id))) || set.ids.some((id) => { const kind = premiseOf(id); return kind !== null && (program.kinds ?? []).includes(premiseSelectionKind(kind)); });
}

/** The lifting sets of a scan's formula groups, with the rows and programs of each: what to ask `--assume`. */
export async function liftingSetsOf(path) {
  const found = new Map();
  const groups = new Map();
  for (const program of await scanPrograms(path)) {
    const seen = new Set();
    for (const { kind, formula, fired } of program.formulas) {
      const key = JSON.stringify([kind, formula, [...fired].sort()]);
      const entry = groups.get(key) ?? { kind, formula, fired: [...fired].sort(), rows: 0, programs: 0 };
      entry.rows += 1;
      if (!seen.has(key)) { seen.add(key); entry.programs += 1; }
      groups.set(key, entry);
    }
  }
  for (const group of groups.values()) {
    for (const set of group.formula ? liftingSets(group.formula, group.fired) : []) {
      const key = set.join(" + ");
      const entry = found.get(key) ?? { ids: set, rows: 0, groups: 0 };
      entry.rows += group.rows;
      entry.groups += 1;
      found.set(key, entry);
    }
  }
  return { sets: [...found.values()].sort((a, b) => b.rows - a.rows || (a.ids.join() < b.ids.join() ? -1 : 1)), groups: [...groups.values()] };
}

async function setsCommand(argv) {
  const found = options(argv, ["scan", "top"]);
  if (!found.scan) throw new Error("--scan <blocker-scan rows.jsonl> is required");
  const { sets } = await liftingSetsOf(found.scan);
  for (const set of sets.slice(0, Number(found.top ?? 30))) process.stdout.write(`${String(set.rows).padStart(8)} rows ${String(set.groups).padStart(3)} groups  ${set.ids.join(" + ")}\n`);
  process.stdout.write(`${sets.length} lifting set(s)\n`);
}

/** `movers`: the moved programs of a grid report as one id manifest per (grid, code), and the commands that run them in Chrome with the shipped checker (scripts/fuzz/rerun-ids.mjs's). */
async function moversCommand(argv) {
  const found = options(argv, ["report", "out", "run-out", "checker-bin", "typefacts"]);
  if (!found.report || !found.out) throw new Error("--report <grid report.json> and --out <manifest dir> are required");
  const { rerunCommand } = await import("./fuzz/rerun-ids.mjs");
  const { GENERATOR_VERSION } = await import("./fuzz/generate.mjs");
  const report = JSON.parse(readFileSync(resolve(found.report), "utf8"));
  const groups = new Map();
  for await (const result of resultLines(report, found.report)) {
    if (!Object.values(result.bySet ?? {}).some((entry) => entry.transitions)) continue;
    const key = `${result.grid}\0${result.code}`;
    const entry = groups.get(key) ?? { grid: result.grid, code: result.code, seed: report.seed, ids: [], contexts: new Set() };
    entry.ids.push(result.program);
    entry.contexts.add(result.context);
    groups.set(key, entry);
  }
  mkdirSync(resolve(found.out), { recursive: true });
  const extra = ["--runtime-diff", "all", ...(found["checker-bin"] ? ["--checker-bin", resolve(found["checker-bin"]), "--typefacts", resolve(found.typefacts ?? "")] : [])];
  const commands = [];
  for (const entry of groups.values()) {
    const manifest = join(resolve(found.out), `${entry.grid}-${entry.code}.json`);
    writeFileSync(manifest, `${JSON.stringify({ generator: GENERATOR_VERSION, seed: String(entry.seed), ids: entry.ids.sort() })}\n`);
    commands.push(rerunCommand({ ...entry, contexts: [...entry.contexts].sort() }, manifest, resolve(found["run-out"] ?? join(found.out, "chrome")), extra).replace(" --confirm-with-chrome", ""));
  }
  writeFileSync(join(resolve(found.out), "movers.sh"), `#!/usr/bin/env bash\nset -u\n${commands.join("\n")}\n`);
  process.stdout.write(`${groups.size} (grid, code) manifest(s), ${[...groups.values()].reduce((total, entry) => total + entry.ids.length, 0)} program(s): ${join(resolve(found.out), "movers.sh")}\n`);
}

/** `judge`: the verdicts of a grid report's moved programs against the Chrome records under a directory. */
async function judgeCommand(argv) {
  const found = options(argv, ["report", "chrome", "out"]);
  if (!found.report || !found.chrome || !found.out) throw new Error("--report <grid report.json>, --chrome <dir of chrome-*.json> and --out <judged.json> are required");
  const report = JSON.parse(readFileSync(resolve(found.report), "utf8"));
  const records = chromeRecords(found.chrome);
  const outcomes = programOutcomes(found.chrome);
  const state = newJudge();
  for await (const result of resultLines(report, found.report)) judgeOne(state, result, records, outcomes);
  const judged = { kind: "what-if", note: "A prediction judged against Chrome's runtime record: line and policy based, not the fuzz judge.", report: found.report, chromeRecords: records.size, ...finishJudge(state) };
  writeFileSync(resolve(found.out), `${JSON.stringify(judged, null, 2)}\n`);
  for (const [name, tally] of Object.entries(judged.sets)) process.stdout.write(`${name}: ${tally.programs} program(s) (${tally.noRecord} without a record: ${JSON.stringify(tally.noRecordBy)}): ${Object.entries(tally.verdicts).map(([verdict, count]) => `${verdict} ${count}`).join(", ")}\n`);
  process.stdout.write(`calibration: ${judged.calibration.agree} of ${judged.calibration.diagnostics} diagnostics (${judged.calibration.programs} programs) read as the oracle covered them; ${judged.calibration.disagree.length} differ\n`);
}

async function grid(argv) {
  const found = options(argv, ["what-if-bin", "checker-bin", "typefacts", "set", "sets-file", "scan", "jobs", "limit", "every", "programs-from", "control", "out", "movers-out", "install", "seed"]);
  if (!found.out || !found.scan) throw new Error("--scan <rows.jsonl> and --out <report.json> are required");
  const sets = setsOf(found);
  const binaries = binariesOf(found);
  const { gridRows } = await import("./fuzz/cover.mjs");
  const { buildRow } = await import("./fuzz/blocker-scan.mjs");
  const { materializeForChecker, checkerEnv } = await import("./fuzz/checker.mjs");
  const install = resolve(found.install ?? join(here, "fuzz", "install"));
  const seed = found.seed ?? "1";
  const scanned = await scanPrograms(found.scan, { kinds: sets.some((set) => set.ids.some((id) => premiseOf(id) !== null)) });
  // A program can move under a set only when one of its rows fires a hazard of it (the seam answers only for a hazard the program has, and a row's `fired` names the ones that
  // veto it), or, for a `Premise(<kind>)` id, carries a row of that cause; every other program is not run. `--programs-from all` runs every program the scan holds under every set instead.
  const wanted = (program, set) => programWantsSet(program, set, found["programs-from"] === "all");
  // `--every K` takes every K-th program of the selection (a spread over the groups, for a pilot); `--limit N` the first N.
  const needed = scanned.filter((program) => sets.some((set) => wanted(program, set))).filter((_, at) => at % Number(found.every ?? 1) === 0).slice(0, found.limit ? Number(found.limit) : undefined);
  const byId = new Map(needed.map((program) => [program.id, program]));
  const rows = [];
  for (const name of new Set(needed.map((program) => program.grid))) for (const row of gridRows(name)) if (byId.has(row.id)) rows.push(row);
  process.stderr.write(`what-if grid: ${needed.length} program(s) with a row firing a hazard of a set or carrying a cause of one (of ${scanned.length} the scan holds), ${rows.length} regenerated\n`);
  const ordinary = ["--format", "json", "--runtime-target", "browser", "--runtime-build", "development", "--rendering", "csr"];
  const feedback = ["--format", "json", "--feedback-facts", "--runtime-target", "browser", "--runtime-build", "development", "--rendering", "csr"];
  // The control (a what-if run that assumes nothing must be the shipped checker's report) runs on `--control N` programs spread over the selection, 200 by default.
  const stride = Math.max(1, Math.floor(rows.length / Number(found.control ?? 200)));
  const resultsFile = `${basename(resolve(found.out), ".json")}.results.jsonl`;
  const tasks = rows.map((row, index) => async () => {
    const built = buildRow(row, seed);
    const dir = materializeForChecker(built, install, built.id);
    try {
      const packaged = Object.keys(built.packages ?? {}).length > 0;
      const mine = sets.filter((set) => wanted(byId.get(row.id), set));
      const program = { id: row.id, cwd: dir, args: ["--project", join(dir, "tsconfig.json"), ...(packaged ? ordinary : feedback)] };
      const result = await measure({ program, sets: mine, binaries, control: index % stride === 0 && found.control !== "0", root: realpathSync(dir), env: { ...checkerEnv({ typefacts: binaries.typefacts }), SOLID_CHECKER_DAEMON: "0" } });
      const full = { ...result, project: `${row.grid}/${row.code}`, grid: row.grid, code: row.code, context: row.context, side: row.side };
      foldOne(state, full);
      const stored = slimResult(full);
      // A program that moved (or failed) is written as soon as it is known, one line, and not kept: the whole sample does not fit one JSON string.
      if (stored.error || Object.values(stored.bySet ?? {}).some((entry) => entry.transitions)) appendFileSync(join(dirname(resolve(found.out)), resultsFile), `${JSON.stringify(stored)}\n`);
      return null;
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
  const state = newFold(sets);
  mkdirSync(dirname(resolve(found.out)), { recursive: true });
  writeFileSync(join(dirname(resolve(found.out)), resultsFile), "");
  await pool(tasks, Number(found.jobs ?? 12));
  const report = { ...header(binaries, { source: "grid", scan: found.scan, sets, programs: rows.length, seed }), ...finishFold(state), resultsFile };
  mkdirSync(dirname(resolve(found.out)), { recursive: true });
  writeFileSync(resolve(found.out), `${JSON.stringify(report, null, 2)}\n`);
  if (found["movers-out"]) writeFileSync(resolve(found["movers-out"]), `${JSON.stringify(Object.fromEntries(Object.entries(report.moved)), null, 1)}\n`);
  process.stdout.write(summaryText(report, sets));
}

// --- Chrome's record against the what-if rows ---------------------------------------------------------------------------------------------------------------------
// The fuzz judge refuses a what-if report (scripts/fuzz/checker.mjs), so a program that moved is compared with Chrome's runtime record directly, with two questions only:
// a row that became a violation: does Chrome report that rule's code at the site? A row that disappeared: is Chrome silent where the row stood, or is what it reported still covered?
// "At the site" is the diagnostic's own line or a line of its stack, in the same file. The record is the runtime oracle's (the `chrome-*.json` a grid run with `--runtime-diff all`
// writes: `diagnostics` with `code`, `file`, `line`, `stack`, and `covering`, the labels of the shipped checker's findings that cover each).

/** The oracle's label of a finding: `App.tsx:12 SC2006 uncertifiable flush-in-action`. */
export const label = ({ file, line, id, kind, rule }) => `${file}:${line} ${id} ${kind} ${rule}`;

const frameAt = (frame) => {
  const match = /^(.*):(\d+):\d+$/.exec(String(frame));
  return match ? { file: match[1], line: Number(match[2]) } : null;
};

/** Whether a diagnostic of the oracle stands at `finding`: its own line, or a line of its stack, in the finding's file. */
export const standsAt = (diagnostic, finding) => [{ file: diagnostic.file, line: diagnostic.line }, ...(diagnostic.stack ?? []).map(frameAt).filter(Boolean)].some((place) => place.file === finding.file && place.line === finding.line);

/** Whether a violation of `rule` answers a diagnostic of `code` (the policy of scripts/fuzz/codes.mjs). */
const claims = (rule, code) => (codeRules[code] ?? []).includes(rule);

/** Whether `findings` cover the diagnostic: one stands at it and is an obligation or a violation of a rule that claims its code. */
const coveredBy = (diagnostic, findings) => findings.some((finding) => standsAt(diagnostic, finding) && (finding.kind === "uncertifiable" || claims(finding.rule, diagnostic.code)));

/** The runtime oracle's rows under `directory` (every `chrome-*.json`, at any depth), by program id; a clean twin (`<id>~clean`) is not a program. */
export function chromeRecords(directory) {
  const rows = new Map();
  const visit = (path) => {
    for (const name of readdirSync(path)) {
      const entry = join(path, name);
      if (statSync(entry).isDirectory()) visit(entry);
      else if (/^chrome-.*\.json$/.test(name)) for (const row of JSON.parse(readFileSync(entry, "utf8")).results ?? []) if (!String(row.id).endsWith("~clean")) rows.set(row.id, row);
    }
  };
  visit(resolve(directory));
  return rows;
}

/** The outcome the grid run gave each program under `directory` (every `programs.jsonl`, at any depth), by id: `typed-away` is a program tsc rejects, which Chrome never ran. */
export function programOutcomes(directory) {
  const outcomes = new Map();
  const visit = (path) => {
    for (const name of readdirSync(path)) {
      const entry = join(path, name);
      if (statSync(entry).isDirectory()) visit(entry);
      else if (name === "programs.jsonl") for (const line of readFileSync(entry, "utf8").split("\n")) if (line) { const row = JSON.parse(line); outcomes.set(row.id, row.outcome); }
    }
  };
  visit(resolve(directory));
  return outcomes;
}

/** How the comparison's own reading of "covered" fares against the oracle's, on the shipped checker's findings: `{ diagnostics, agree, disagree: [..] }`. */
export function calibration(oracle) {
  const result = { diagnostics: 0, agree: 0, disagree: [] };
  for (const diagnostic of oracle.diagnostics ?? []) {
    result.diagnostics += 1;
    const mine = coveredBy(diagnostic, (oracle.findings ?? []).map((finding) => ({ ...finding, file: finding.file ?? "App.tsx" })));
    if (mine === ((diagnostic.covering ?? []).length > 0)) result.agree += 1;
    else result.disagree.push({ code: diagnostic.code, line: diagnostic.line, oracle: diagnostic.verdict, covering: diagnostic.covering ?? [] });
  }
  return result;
}

const violationVerdict = (violation, diagnostics) => {
  if (!ruleHasRuntimeCounterpart(violation.rule)) return "violation-unjudgeable";
  const claimed = new Set(codesForRule(violation.rule));
  const here = diagnostics.filter((diagnostic) => standsAt(diagnostic, violation));
  if (here.some((diagnostic) => claimed.has(diagnostic.code))) return "violation-confirmed";
  return here.length ? "WRONG-RULE" : "FALSE-POSITIVE";
};

/**
 * A moved program under one set, against Chrome's record of it (`oracle`, a runtime-oracle row): one verdict per row that moved and per new violation.
 * `toClean`: `clean-silent` (Chrome reported nothing the row covered), `clean-covered-elsewhere` (what it covered is still covered by what the what-if build reports) or `FALSE-CLEAN`.
 * `toV`/`toVOther`: for each violation the what-if build added at the row's span (`fresh`), `violation-confirmed`, `WRONG-RULE` (Chrome reports another code there), `FALSE-POSITIVE` (Chrome
 * reports nothing there) or `violation-unjudgeable` (a rule no runtime diagnostic answers). A new violation, at a span the shipped checker had no finding at, is judged the same way.
 * @param {{ moved?: object[], new?: object[], now?: object[] }} entry a stored (slim) entry
 */
export function judgeMoves(entry, oracle) {
  const diagnostics = oracle.diagnostics ?? [];
  const verdicts = [];
  for (const row of entry.moved ?? []) {
    const covered = diagnostics.filter((diagnostic) => (diagnostic.covering ?? []).includes(label(row)));
    if (row.outcome === "toClean") {
      const lost = covered.filter((diagnostic) => !coveredBy(diagnostic, entry.now ?? []));
      verdicts.push({ group: row.group, row: label(row), verdict: covered.length === 0 ? "clean-silent" : lost.length ? "FALSE-CLEAN" : "clean-covered-elsewhere", codes: [...new Set((lost.length ? lost : covered).map((diagnostic) => diagnostic.code))] });
    } else if (row.outcome === "toV" || row.outcome === "toVOther") {
      // The violations the what-if build added at the span: a violation the shipped checker held there already is its own, judged by the grid run's own comparison, not a move of the assumption
      // (a line stored before `fresh` existed has every violation at the span).
      for (const violation of row.fresh ?? row.now.filter((finding) => finding.kind === "violation")) verdicts.push({ group: row.group, row: label(row), now: label(violation), verdict: violationVerdict(violation, diagnostics) });
    }
  }
  for (const finding of entry.new ?? []) if (finding.kind === "violation") verdicts.push({ group: "new", now: label(finding), verdict: violationVerdict(finding, diagnostics) });
  return verdicts;
}

/** The state a judge starts from. */
export const newJudge = () => ({ sets: {}, calibration: { programs: 0, diagnostics: 0, agree: 0, disagree: [] }, seen: new Set() });

/** Adds one stored program result (its moved sets) to a judge state, against `records` (a program with no record says why, from the grid run's own outcome, and is not guessed). */
export function judgeOne(state, result, records, outcomes = new Map()) {
  const oracle = records.get(result.program);
  for (const [name, entry] of Object.entries(result.bySet ?? {})) {
    if (!entry.transitions) continue;
    const tally = (state.sets[name] ??= { programs: 0, noRecord: 0, noRecordBy: {}, verdicts: {}, groups: {}, problems: [] });
    tally.programs += 1;
    if (!oracle) {
      tally.noRecord += 1;
      const why = outcomes.get(result.program) ?? "no row";
      tally.noRecordBy[why] = (tally.noRecordBy[why] ?? 0) + 1;
      continue;
    }
    for (const verdict of judgeMoves(entry, oracle)) {
      tally.verdicts[verdict.verdict] = (tally.verdicts[verdict.verdict] ?? 0) + 1;
      const inGroup = (tally.groups[verdict.group] ??= {});
      inGroup[verdict.verdict] = (inGroup[verdict.verdict] ?? 0) + 1;
      if (/^[A-Z-]+$/.test(verdict.verdict) && verdict.verdict !== "clean-silent") tally.problems.push({ program: result.program, grid: result.grid, code: result.code, context: result.context, side: result.side, ...verdict });
    }
  }
  if (oracle && !state.seen.has(result.program)) {
    state.seen.add(result.program);
    const one = calibration(oracle);
    state.calibration.programs += 1;
    state.calibration.diagnostics += one.diagnostics;
    state.calibration.agree += one.agree;
    state.calibration.disagree.push(...one.disagree.map((item) => ({ program: result.program, ...item })));
  }
}

/** The verdicts of a judge state: `{ sets, calibration }`. */
export const finishJudge = (state) => ({ sets: state.sets, calibration: state.calibration });

/** Per set, the verdicts of every moved program of a report that holds its results (`results`) against `records`. */
export function judgeReport(report, records, outcomes = new Map()) {
  const state = newJudge();
  for (const result of report.results ?? []) judgeOne(state, result, records, outcomes);
  return finishJudge(state);
}

/**
 * The stored results of a report, one at a time: the report's own `results`, or the lines of the file it names (`resultsFile`, beside it, one program a line: a corpus of 35,000 programs
 * does not fit one JSON string).
 */
export async function* resultLines(report, reportPath) {
  if (report.results) { yield* report.results; return; }
  if (!report.resultsFile) return;
  for await (const line of createInterface({ input: createReadStream(resolve(dirname(resolve(reportPath)), report.resultsFile)) })) if (line) yield JSON.parse(line);
}

/**
 * ADR 0965: the sets of a corpus report ranked by what they lift, the most first: `[{ name, ids, sites, rows, toV, programs }]`. `sites` are the sites that stop holding an obligation
 * (`uLost`, and the ones that become a violation, `uToV`), `rows` the uncertifiable rows that leave or become a violation in the groups that moved, `programs` the projects it moved. A count,
 * never safety: the rows leave as clean, and the corpus has no runtime to say a site must not.
 */
export function rankSets(report, judged = null) {
  const total = (report.baselines ?? []).reduce((sum, base) => sum + (base.rows?.u ?? 0), 0);
  return report.sets.map(({ name, ids }) => {
    const counts = report.arms[name]?.counts ?? {};
    const groups = Object.values(report.groups[name] ?? {});
    const rows = groups.reduce((sum, entry) => sum + entry.toClean + entry.toV + entry.toVOther, 0);
    return { name, ids, sites: (counts.uLost ?? 0) + (counts.uToV ?? 0), rows, toV: groups.reduce((sum, entry) => sum + entry.toV + entry.toVOther, 0), programs: report.moved?.[name]?.length ?? 0, share: total ? rows / total : 0, chrome: chromeOf(judged, name) };
  }).sort((a, b) => b.rows - a.rows || b.sites - a.sites || (a.name < b.name ? -1 : 1));
}

/**
 * ADR 0965: what Chrome said of the programs a set moved (`judge`'s report, `sets[name]`): the programs with a record, those where the lifted site is silent or covered by another row, and the
 * false cleans (a diagnostic of Chrome's at a site the set would leave clean). `null` where the judged report holds no such set.
 */
export function chromeOf(judged, name) {
  const tally = judged?.sets?.[name];
  if (!tally) return null;
  const verdicts = tally.verdicts ?? {};
  return { judged: tally.programs - tally.noRecord, silent: verdicts["clean-silent"] ?? 0, covered: verdicts["clean-covered-elsewhere"] ?? 0, falseClean: verdicts["FALSE-CLEAN"] ?? 0, other: Object.entries(verdicts).filter(([verdict]) => !["clean-silent", "clean-covered-elsewhere", "FALSE-CLEAN"].includes(verdict)).reduce((sum, [, count]) => sum + count, 0) };
}

/** The ranking as text: one line a set, with the share of the corpus's uncertifiable rows and, with a judged grid report, what Chrome said. */
export function rankText(report, top = 40, judged = null) {
  const total = (report.baselines ?? []).reduce((sum, base) => sum + (base.rows?.u ?? 0), 0);
  const lines = [`what-if ranking (${report.source}): ${report.programs} program(s), ${total} uncertifiable rows under the shipped checker; control ${report.control.same} of ${report.control.programs} identical${judged ? `; Chrome: ${judged.chromeRecords} record(s)` : ""}`];
  for (const entry of rankSets(report, judged).slice(0, top)) {
    const chrome = judged ? (entry.chrome ? ` | chrome ${String(entry.chrome.judged).padStart(5)} judged, false clean ${String(entry.chrome.falseClean).padStart(4)}, silent ${entry.chrome.silent}, covered elsewhere ${entry.chrome.covered}` : " | chrome: no program of the grid") : "";
    lines.push(`${String(entry.rows).padStart(8)} rows (${(100 * entry.share).toFixed(1).padStart(5)}%) ${String(entry.sites).padStart(7)} sites ${String(entry.programs).padStart(3)} program(s) ${entry.toV ? `toV ${entry.toV} ` : ""}${entry.ids.join(" + ")}${chrome}`);
  }
  return `${lines.join("\n")}\n`;
}

async function rankCommand(argv) {
  const found = options(argv, ["report", "top", "judged"]);
  if (!found.report) throw new Error("--report <corpus report.json> is required");
  const report = JSON.parse(readFileSync(resolve(found.report), "utf8"));
  if (report.kind !== "what-if" || report.source !== "corpus") throw new Error("rank reads a corpus what-if report");
  const judged = found.judged ? JSON.parse(readFileSync(resolve(found.judged), "utf8")) : null;
  process.stdout.write(rankText(report, Number(found.top ?? 40), judged));
}

/** The summary printed after a run: per set, the harness's counts and the groups that moved. */
export function summaryText(report, sets) {
  const lines = [`what-if (${report.source}): ${report.programs} program(s), ${report.errors} error(s); control: ${report.control.same} of ${report.control.programs} identical to the shipped checker`];
  for (const { name, ids } of sets) {
    const counts = report.arms[name]?.counts ?? {};
    lines.push(`${name} = ${ids.join(" + ")}: ${Object.entries(counts).filter(([, count]) => count).map(([kind, count]) => `${kind} ${count}`).join(", ") || "nothing moves"} (${report.arms[name]?.witnesses.length ?? 0} program(s) move)`);
    for (const [group, entry] of Object.entries(report.groups[name]).filter(([, entry]) => entry.toV + entry.toVOther + entry.toClean > 0).sort((a, b) => b[1].rows - a[1].rows).slice(0, 12)) {
      lines.push(`    ${String(entry.rows).padStart(7)} rows: toV ${entry.toV + entry.toVOther}, toClean ${entry.toClean}, stillU ${entry.stillU + entry.blockedChanged}, other ${entry.toUOther}${entry.besideV ? `, besideV ${entry.besideV}` : ""}  ${group}`);
    }
  }
  return `${lines.join("\n")}\n`;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const [command, ...rest] = process.argv.slice(2);
  const commands = { corpus, grid, sets: setsCommand, movers: moversCommand, judge: judgeCommand, rank: rankCommand };
  if (!commands[command]) {
    process.stderr.write("usage: node scripts/what-if.mjs corpus|grid|sets ... (see the header of the script)\n");
    process.exit(64);
  }
  try {
    await commands[command](rest);
  } catch (error) {
    process.stderr.write(`what-if: ${error.message}\n`);
    process.exit(2);
  }
}
