#!/usr/bin/env node
// The recall matrix of the misuse profile (misuse.mjs): for every code a rule claims, every context the template targets, N programs
// that misuse one Solid API at one marked site. Each program is judged at its site, against the runtime that confirms the misuse:
//
//   programs         generated for the cell
//   typed-away       tsc rejects the program (invariant 1: the checker stays silent; not a gap)
//   harness          the checker or the runtime run failed (not evidence)
//   not-fired        the program type-checks and the runtime did not report the code at the site: a generator problem, not the checker's
//   confirmed        the runtime reported the code at the site (the only programs that count in the checker columns)
//     agree          a violation of a rule that claims the code covers the site
//     wrong-rule     a violation of another rule covers the site and none of the claiming rules does
//     recall-gap     only uncertifiable findings cover the site (the checker declined)
//     false-clean    no finding covers the site: the checker is silent where Solid speaks
//
// Chrome (the authority) confirms up to --chrome-sample agree programs per cell and every wrong-rule and false-clean program; one
// representative per false-clean cell is shrunk. `--baseline <matrix.json>` gates: a cell whose agree count drops or whose false-clean
// count rises, for the same seeds, fails the run (exit 1). `--gate <baseline.json> <current.json>` compares two recorded matrices.
//
//   node scripts/fuzz/matrix.mjs --out <fresh dir> --seed 1 --per-cell 20 [--codes A,B] [--contexts a,b] [--confirm-with-chrome]
//     [--chrome-sample 3] [--shrink] [--baseline matrix.json] [--checker-label <commit>] [--all-cores]
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { codeRules, ignoredRules, withholdingRules } from "./codes.mjs";
import { runtimeEvents } from "./classify.mjs";
import { DEFAULT_ORACLE_INSTALL, runChrome } from "./chrome.mjs";
import { buildCase, writeCase } from "./emit.mjs";
import { GENERATOR_VERSION, generateProgram } from "./generate.mjs";
import { MISUSE_CODES, SPEC, TEMPLATE_CODES } from "./misuse.mjs";
import { isSite, walk } from "./model.mjs";
import { evaluate } from "./pipeline.mjs";
import { programSeed } from "./rng.mjs";
import { shrink } from "./shrink.mjs";
import { legalTwin } from "./falseclean.mjs";
import { attachDaemon, configFrom, parseArgs } from "./run.mjs";
import { assertFindingsMapped, materializeForChecker } from "./checker.mjs";
import { TYPE_ERROR_REFUSAL_SHARE, requireTypeChecker, resolveTsc, typeErrorRefusal, typecheck } from "./typecheck.mjs";

export const OUTCOMES = ["typed-away", "harness", "not-fired", "agree", "wrong-rule", "recall-gap", "false-clean"];
const CONFIRMED = ["agree", "wrong-rule", "recall-gap", "false-clean"];

// ---------------------------------------------------------------------------------------------------------------------
// Judging one program at its site

const inLines = (finding, lines) => lines.some((line) => finding.line <= line && line <= finding.endLine);

/** The lines that belong to the site: where the misuse call is written, and the line of a wrapper's inner call. */
export const regionOf = (site) => [site.line, ...(site.aux ?? [])].filter((line) => Number.isInteger(line));

/**
 * @param {{ code: string, site: object, typecheck: object, checker: object, runtime: object }} input
 * @returns {{ outcome: string, detail?: object }}
 */
export function judge({ code, site, typecheck: typed, checker, runtime }) {
  if (typed && typed.ok === false) return { outcome: "typed-away", detail: { error: typed.errors?.[0] } };
  if (!checker?.ok) return { outcome: "harness", detail: { why: `checker: ${String(checker?.error ?? "not run").slice(0, 120)}` } };
  if (!runtime) return { outcome: "harness", detail: { why: "no runtime record" } };
  const region = regionOf(site);
  // The coded errors the page threw: `[CODE] message` is the dev build's own tag (Chrome reports some codes only this way).
  const thrownTags = (runtime.errors ?? []).flatMap((error) => {
    const tag = /^\[([A-Z][A-Z_]+)\]/.exec(String(error.message ?? ""));
    return tag ? [{ code: tag[1], source: "thrown", positions: error.positions ?? [] }] : [];
  });
  const known = runtimeEvents(runtime);
  const events = [...known, ...thrownTags.filter((tag) => !known.some((event) => event.code === tag.code && event.source === "thrown"))].filter((event) => event.code === code);
  const attributed = (event) => event.positions.filter((position) => position.file === "App.tsx" || position.file === undefined);
  const atSite = events.filter((event) => attributed(event).some((position) => region.includes(position.line)));
  // A diagnostic with no authored position at all (or, for a placeless code, any position): Solid spoke and the harness cannot say
  // where. With one site in the file that is this site. Chrome lists such codes in `unattributed`.
  const unplaced = events.filter((event) => SPEC[code].placeless || !attributed(event).length);
  const unattributed = (runtime.unattributed ?? []).map(String).includes(code);
  if (!atSite.length && !unplaced.length && !unattributed) {
    if (runtime.failure && !events.length) return { outcome: "harness", detail: { why: `runtime: ${String(runtime.failure.message).slice(0, 120)}` } };
    return { outcome: "not-fired", detail: { reported: [...new Set(runtimeEvents(runtime).map((event) => event.code))], misplaced: events.map((event) => attributed(event).map((p) => p.line)) } };
  }
  const placed = atSite.length ? "placed" : "unplaced";
  const rules = codeRules[code] ?? [];
  const relevant = checker.findings.filter((finding) => !ignoredRules.includes(finding.rule));
  const claims = (finding) => rules.includes(finding.rule) || withholdingRules.includes(finding.rule);
  const covering = relevant.filter((finding) => inLines(finding, region));
  // The checker anchors a finding at the call that runs the misuse (a closure's call, a helper's call), not always at the misuse
  // line itself. In a one-site program a claiming finding anywhere is this misuse's: it is an agree (or a gap) with `remote` set.
  const claimingViolation = relevant.find((finding) => finding.kind === "violation" && rules.includes(finding.rule) && inLines(finding, region))
    ?? relevant.find((finding) => finding.kind === "violation" && rules.includes(finding.rule));
  const remote = Boolean(claimingViolation) && !inLines(claimingViolation, region);
  const violations = covering.filter((finding) => finding.kind === "violation");
  const stray = relevant.filter((finding) => finding.kind === "violation" && !rules.includes(finding.rule) && !inLines(finding, region));
  const detail = { placed, findings: covering.map((finding) => `${finding.rule}/${finding.kind}@${finding.line}`), ...(remote ? { remote: `${claimingViolation.rule}@${claimingViolation.line}` } : {}), ...(stray.length ? { stray: stray.map((finding) => `${finding.rule}@${finding.line}`) } : {}) };
  if (claimingViolation) return { outcome: "agree", detail };
  if (violations.length) return { outcome: "wrong-rule", detail: { ...detail, rules: [...new Set(violations.map((finding) => finding.rule))] } };
  if (covering.length) return { outcome: "recall-gap", detail };
  const claimingElsewhere = relevant.find((finding) => claims(finding));
  if (claimingElsewhere) return { outcome: "recall-gap", detail: { ...detail, remote: `${claimingElsewhere.rule}/${claimingElsewhere.kind}@${claimingElsewhere.line}` } };
  return { outcome: "false-clean", detail };
}

/** The misuse site of a built case: its only `use` site. */
export const siteOf = (built) => built.sites.find((site) => site.code) ?? built.sites[0];

// ---------------------------------------------------------------------------------------------------------------------
// The cells

export const fnv = (text) => { let hash = 0x811c9dc5; for (const ch of text) hash = Math.imul(hash ^ ch.charCodeAt(0), 0x01000193) >>> 0; return hash; };
/** A program's seed depends on the run seed, its cell and its index in the cell, so a cell's programs do not move when others are added. */
export const cellSeed = (base, code, context, index) => programSeed((Number(base) ^ fnv(`${code}/${context}`)) >>> 0, index);

export function cellList({ codes, contexts } = {}) {
  const cells = [];
  for (const code of TEMPLATE_CODES) {
    if (codes && !codes.includes(code)) continue;
    for (const context of SPEC[code].contexts) if (!contexts || contexts.includes(context)) cells.push({ code, context });
  }
  return cells;
}

export function generateCell(options, { code, context }, index) {
  const seed = cellSeed(options.seed, code, context, index);
  const program = generateProgram(seed, { profile: "misuse", misusePlan: { code, context } });
  return { program, built: buildCase(program, { id: `mx-${code.toLowerCase().replaceAll("_", "-")}-${context}-${index}` }) };
}

const emptyTally = () => Object.fromEntries(OUTCOMES.map((outcome) => [outcome, 0]));
const tally = (rows) => rows.reduce((counts, row) => { counts[row.outcome] += 1; return counts; }, emptyTally());

// ---------------------------------------------------------------------------------------------------------------------
// Chrome

/** Chrome's records for evaluated values: the twin, judged against the clean case's findings, which run beside it in the same oracle call. */
async function chromeRecords(values, config, options, out) {
  const chromeOptions = { out, install: config.install, bin: config.bin, typefacts: config.typefacts, browser: options.browser, concurrency: options.concurrency };
  const cases = values.map((value) => ({ id: value.built.id, dir: value.caseDir, sites: value.built.sites }));
  return runChrome(cases, { ...chromeOptions, withClean: true });
}

/** Chrome's outcome for a program, and whether it equals the fast path's. */
function judgeChrome(row, record) {
  if (!record || record.runtime?.failure || !record.checker?.ok || record.checker.perturbed?.length) return { status: "chrome-inconclusive", why: record?.runtime?.failure?.message ?? record?.checker?.error ?? record?.checker?.perturbed?.[0] ?? "no record" };
  const { outcome, detail } = judge({ code: row.code, site: row.site, typecheck: { ok: true }, checker: record.checker, runtime: record.runtime });
  return { status: outcome === row.outcome ? "same" : outcome === "not-fired" ? "not-fired-in-chrome" : "differs", outcome, detail };
}

// ---------------------------------------------------------------------------------------------------------------------
// Shrinking a false-clean representative

async function shrinkRepresentative(row, value, config, options, out) {
  const target = { code: row.code };
  let counter = 0;
  const evaluateAll = async (programs, tag) => {
    const entries = programs.map((program) => { counter += 1; return { program, built: buildCase(program, { id: `${row.id}-${tag}${counter}` }) }; });
    const results = await evaluate(entries, { ...config, casesDir: join(out, "shrink") });
    return entries.map(({ built }) => ({ built, value: results.get(built.id) }));
  };
  const holds = ({ built, value: result }) => {
    const site = siteOf(built);
    if (!site || !result?.checker) return false;
    const verdict = judge({ code: target.code, site, typecheck: result.typecheck, checker: result.checker, runtime: result.runtime });
    return verdict.outcome === "false-clean";
  };
  const judgeBatch = async (programs) => (await evaluateAll(programs, "fc")).map(holds);
  // The control: the program with its misuse replaced by a literal reports neither the code nor a finding.
  const control = async (program) => {
    const twin = legalTwin(program);
    if (twin === null) return true;
    const [{ value: result }] = await evaluateAll([twin], "twin");
    if (!result?.runtime || !result.checker?.ok || !result.typecheck?.ok) return false;
    return !runtimeEvents(result.runtime).some((event) => event.code === target.code);
  };
  const cheap = (program) => [...walk(program)].some(({ node }) => isSite(node));
  const reduced = await shrink(value.program, async (program) => (await judgeBatch([program]))[0], { budget: options.budget, cheap, control, judgeBatch, batchSize: options.shrinkBatch ?? 8 });
  const built = buildCase(reduced.program, { id: `${row.id}-min`, note: `shrunk from misuse matrix program ${row.id} (${reduced.accepted} reductions, ${reduced.attempts} runs), false clean ${target.code}` });
  const dir = writeCase(join(out, "repros", createHash("sha1").update(built.text).digest("hex").slice(0, 10)), built);
  return { dir, size: built.text.split("\n").length, text: built.text, reductions: reduced.accepted, runs: reduced.attempts, program: reduced.program, built };
}

// ---------------------------------------------------------------------------------------------------------------------
// Report

/** Builds the recorded matrix (JSON) from the per-program rows. */
export function buildMatrix(rows, { options, header, typedAway }) {
  const cells = {};
  for (const row of rows) {
    const key = `${row.code}|${row.context}`;
    const cell = cells[key] ??= { code: row.code, context: row.context, rows: [] };
    cell.rows.push(row);
  }
  const out = {};
  for (const [key, cell] of Object.entries(cells)) {
    const counts = tally(cell.rows);
    const confirmed = CONFIRMED.reduce((sum, name) => sum + counts[name], 0);
    const by = (field) => Object.fromEntries([...new Set(cell.rows.map((row) => row[field]))].sort().map((value) => [value, tally(cell.rows.filter((row) => row[field] === value))]));
    out[key] = {
      code: cell.code, context: cell.context, programs: cell.rows.length, ...counts, confirmed,
      agreeRemote: cell.rows.filter((row) => row.outcome === "agree" && row.detail?.remote).length,
      byBinding: by("binding"), byLayer: by("layer"),
      chrome: chromeSummary(cell.rows),
      verdicts: Object.fromEntries(cell.rows.map((row) => [String(row.seed), row.outcome])),
      ...(cell.rows.find((row) => row.minimal) ? { minimal: cell.rows.find((row) => row.minimal).minimal } : {}),
    };
  }
  const total = tally(rows);
  // Violations away from the site, per rule: they need a look (a template's second defect, or a false positive).
  const strays = {};
  for (const row of rows) for (const label of row.detail?.stray ?? []) { const rule = label.split("@")[0]; strays[rule] = [...(strays[rule] ?? []), row.id]; }
  return {
    strays: Object.fromEntries(Object.entries(strays).map(([rule, ids]) => [rule, { programs: ids.length, examples: ids.slice(0, 3) }])),
    generator: GENERATOR_VERSION, profile: "misuse", seed: options.seed, perCell: options.perCell, checker: header, total,
    confirmed: CONFIRMED.reduce((sum, name) => sum + total[name], 0), typedAway, cells: out,
  };
}

function chromeSummary(rows) {
  const checked = rows.filter((row) => row.chrome);
  if (!checked.length) return null;
  const count = (status) => checked.filter((row) => row.chrome.status === status).length;
  return { checked: checked.length, same: count("same"), differs: count("differs"), notFiredInChrome: count("not-fired-in-chrome"), inconclusive: count("chrome-inconclusive") };
}

const pad = (text, width) => String(text).padEnd(width);

/** Markdown: the code x context table (agree/confirmed per cell, with the gap kinds), then the per-code status. */
export function markdown(matrix, statuses) {
  const keys = Object.values(matrix.cells);
  const contexts = [...new Set(keys.map((cell) => cell.context))];
  const codes = [...new Set(keys.map((cell) => cell.code))];
  const lines = [];
  lines.push(`# Misuse recall matrix`, "");
  lines.push(`Generator ${matrix.generator}, seed ${matrix.seed}, ${matrix.perCell} programs per cell. Checker: ${matrix.checker.label ?? "?"}${matrix.checker.bin ? ` (${matrix.checker.bin})` : ""}.`, "");
  lines.push(`Each cell: \`agree/confirmed\`; then \`W\` wrong-rule, \`G\` recall-gap, \`F\` false-clean, \`n\` not-fired, \`t\` typed-away, \`h\` harness. \`†\`: Chrome reproduced none of the sampled programs of the cell. Blank: not a context of that code. Only runtime-confirmed programs count (confirmed = the runtime reported the code at the site).`, "");
  lines.push(`| code | ${contexts.join(" | ")} |`, `| --- | ${contexts.map(() => "---").join(" | ")} |`);
  for (const code of codes) {
    const cells = contexts.map((context) => {
      const cell = matrix.cells[`${code}|${context}`];
      if (!cell) return "";
      const extras = [["W", cell["wrong-rule"]], ["G", cell["recall-gap"]], ["F", cell["false-clean"]], ["n", cell["not-fired"]], ["t", cell["typed-away"]], ["h", cell.harness]].filter(([, n]) => n).map(([k, n]) => `${k}${n}`).join(" ");
      // A dagger: Chrome did not reproduce any of the sampled programs of this cell (the fast path's evidence stands alone).
      const unconfirmed = cell.chrome && cell.chrome.checked > 0 && cell.chrome.same === 0 ? " †" : "";
      return `${cell.agree}/${cell.confirmed}${extras ? ` ${extras}` : ""}${unconfirmed}`;
    });
    lines.push(`| ${code} | ${cells.join(" | ")} |`);
  }
  const t = matrix.total;
  lines.push("", `Totals: ${Object.entries(t).map(([k, v]) => `${k} ${v}`).join(", ")}; runtime-confirmed ${matrix.confirmed}.`, "");
  if (statuses) {
    lines.push(`## Per code`, "", `| code | status | agree | wrong-rule | recall-gap | false-clean | not-fired | why |`, `| --- | --- | --- | --- | --- | --- | --- | --- |`);
    for (const s of statuses) lines.push(`| ${s.code} | ${s.status} | ${s.agree ?? "-"} | ${s.wrongRule ?? "-"} | ${s.recallGap ?? "-"} | ${s.falseClean ?? "-"} | ${s.notFired ?? "-"} | ${s.why ?? ""} |`);
    lines.push("");
  }
  const fc = keys.filter((cell) => cell["false-clean"]);
  if (fc.length) {
    lines.push(`## False-clean cells`, "");
    for (const cell of fc) lines.push(`- ${cell.code} / ${cell.context}: ${cell["false-clean"]} of ${cell.confirmed}${cell.minimal ? `; minimal (${cell.minimal.size} lines) ${cell.minimal.repro}` : ""}`);
    lines.push("");
  }
  const wr = keys.filter((cell) => cell["wrong-rule"]);
  if (wr.length) {
    lines.push(`## Wrong-rule cells`, "");
    for (const cell of wr) lines.push(`- ${cell.code} / ${cell.context}: ${cell["wrong-rule"]} of ${cell.confirmed}`);
    lines.push("");
  }
  return `${lines.join("\n")}\n`;
}

/** The status of each code from its cells. */
export function codeStatuses(matrix) {
  const out = [];
  for (const code of MISUSE_CODES) {
    const spec = SPEC[code];
    if (spec.typedAway) { out.push({ code, status: "typed-away", why: matrix.typedAway?.[code]?.error ?? spec.why }); continue; }
    const cells = Object.values(matrix.cells).filter((cell) => cell.code === code);
    if (!cells.length) { out.push({ code, status: "not-run" }); continue; }
    const sum = (field) => cells.reduce((n, cell) => n + cell[field], 0);
    const agree = sum("agree"); const confirmed = sum("confirmed");
    const wrongRule = sum("wrong-rule"); const recallGap = sum("recall-gap"); const falseClean = sum("false-clean"); const notFired = sum("not-fired");
    const status = !confirmed ? "untriggered" : agree === confirmed ? "proven" : agree ? "gaps" : "no-recall";
    out.push({ code, status, agree: `${agree}/${confirmed}`, wrongRule, recallGap, falseClean, notFired });
  }
  return out;
}

// ---------------------------------------------------------------------------------------------------------------------
// The gate

/** Outcomes that judge nothing about the checker: the type check rejected the program, or the run failed (the machine, not the program). */
const UNJUDGED = new Set(["typed-away", "harness"]);

/**
 * Compares a recorded matrix with a baseline for the same seeds. A cell fails when a program that was judged moves out of `agree`
 * (to a judged outcome) or into `false-clean`; a baseline cell missing from the current matrix fails too. Different seeds, per-cell
 * counts or generator versions are refused, and so is a current run in which most programs are unjudged.
 *
 * A baseline `typed-away` or `harness` is a machine-load artifact, not a verdict: a program that moves out of it is reported as
 * `baselineUnjudged` and never as a regression. A program that moves into one of them is reported as `infrastructure` (a warning:
 * the run, not the checker, changed). Neither fails the gate.
 * @returns {{ ok: boolean, refused?: string, regressions: object[], improvements: object[], baselineUnjudged: object[], infrastructure: object[] }}
 */
export function gate(baseline, current) {
  const empty = { regressions: [], improvements: [], baselineUnjudged: [], infrastructure: [] };
  for (const field of ["seed", "perCell", "generator"]) {
    if (baseline[field] !== current[field]) return { ok: false, refused: `the matrices differ in ${field} (${baseline[field]} vs ${current[field]}): the gate compares the same programs`, ...empty };
  }
  const now = current.total ?? {};
  const all = Object.values(now).reduce((sum, n) => sum + n, 0);
  const unjudged = (now["typed-away"] ?? 0) + (now.harness ?? 0);
  if (all && unjudged / all > TYPE_ERROR_REFUSAL_SHARE) return { ok: false, refused: `${unjudged} of ${all} programs of the current run are typed-away or harness: it judged nothing (the limit is ${TYPE_ERROR_REFUSAL_SHARE * 100}%)`, ...empty };
  const regressions = []; const improvements = []; const baselineUnjudged = []; const infrastructure = [];
  for (const [key, base] of Object.entries(baseline.cells)) {
    const cell = current.cells[key];
    if (!cell) { regressions.push({ cell: key, why: "cell missing from the current matrix" }); continue; }
    const seeds = [...new Set([...Object.keys(base.verdicts), ...Object.keys(cell.verdicts)])];
    const list = (keep) => seeds.filter((seed) => keep(base.verdicts[seed], cell.verdicts[seed])).map((seed) => `${seed}: ${base.verdicts[seed] ?? "missing"} -> ${cell.verdicts[seed] ?? "missing"}`);
    const lost = list((was, is) => was === "agree" && is !== "agree" && !UNJUDGED.has(is));
    const gained = list((was, is) => is === "false-clean" && was !== "false-clean" && !UNJUDGED.has(was));
    if (lost.length) regressions.push({ cell: key, why: `agree ${base.agree} -> ${cell.agree}`, programs: lost });
    if (gained.length) regressions.push({ cell: key, why: `false-clean ${base["false-clean"]} -> ${cell["false-clean"]}`, programs: gained });
    const fromUnjudged = list((was, is) => UNJUDGED.has(was) && is !== was);
    if (fromUnjudged.length) baselineUnjudged.push({ cell: key, programs: fromUnjudged });
    const intoUnjudged = list((was, is) => UNJUDGED.has(is) && !UNJUDGED.has(was));
    if (intoUnjudged.length) infrastructure.push({ cell: key, programs: intoUnjudged });
    if (cell.agree > base.agree) improvements.push({ cell: key, why: `agree ${base.agree} -> ${cell.agree}` });
    if (cell["false-clean"] < base["false-clean"]) improvements.push({ cell: key, why: `false-clean ${base["false-clean"]} -> ${cell["false-clean"]}` });
  }
  return { ok: regressions.length === 0, regressions, improvements, baselineUnjudged, infrastructure };
}

function reportGate(result) {
  if (result.refused) { console.error(`gate refused: ${result.refused}`); return 2; }
  for (const item of result.improvements) console.error(`gate: improved ${item.cell}: ${item.why}`);
  for (const item of result.baselineUnjudged ?? []) console.error(`gate: baseline unjudged ${item.cell} (not a regression, the baseline was typed-away or harness):\n    ${item.programs.slice(0, 5).join("\n    ")}`);
  for (const item of result.infrastructure ?? []) console.error(`gate: INFRASTRUCTURE WARNING ${item.cell}: ${item.programs.length} program(s) are now typed-away or harness (the run is suspect, not the checker):\n    ${item.programs.slice(0, 5).join("\n    ")}`);
  for (const item of result.regressions) console.error(`gate: REGRESSION ${item.cell}: ${item.why}${item.programs?.length ? `\n    ${item.programs.slice(0, 5).join("\n    ")}` : ""}`);
  console.error(result.ok ? "gate: ok" : `gate: ${result.regressions.length} regression(s)`);
  return result.ok ? 0 : 1;
}

// ---------------------------------------------------------------------------------------------------------------------
// Main

const usage = `usage: node scripts/fuzz/matrix.mjs --out <fresh dir> [--seed N] [--per-cell N] [--codes A,B] [--contexts a,b]
  [--checker-bin <solid-checker>] [--typefacts <solid-typefacts>] [--checker-label <commit>] [--tsc <tsrs>] [--all-cores]
  [--confirm-with-chrome [--chrome-sample 3] [--chrome-false-clean 4] [--browser <chromium>]] [--shrink [--budget N]]
  [--baseline <matrix.json>]            fail when a judged program leaves agree or enters false-clean (same seeds); moves out of or into typed-away/harness are reported, not failed
  node scripts/fuzz/matrix.mjs --gate <baseline.json> <current.json>`;

const CHUNK = 150;

export async function runMatrix(options) {
  const out = resolve(options.out);
  if (existsSync(out)) throw new Error(`use a fresh --out path (${out} exists)`);
  requireTypeChecker(resolveTsc(options.tsc));
  mkdirSync(out, { recursive: true });
  const config = attachDaemon(configFrom({ ...options, profile: "misuse" }, out), options);
  try {
    return await sweep(options, config, out);
  } finally {
    await config.fast?.close();
  }
}

async function sweep(options, config, out) {
  const cells = cellList({ codes: options.codes?.split(","), contexts: options.contexts?.split(",") });
  const entries = [];
  for (const cell of cells) for (let index = 0; index < options.perCell; index += 1) entries.push({ cell, index, ...generateCell(options, cell, index) });
  console.error(`matrix: ${cells.length} cells, ${entries.length} programs (${TEMPLATE_CODES.length} codes); evaluating`);
  const rows = []; const values = new Map();
  const started = Date.now();
  for (let at = 0; at < entries.length; at += CHUNK) {
    const chunk = entries.slice(at, at + CHUNK);
    const results = await evaluate(chunk.map(({ program, built }) => ({ program, built })), config);
    for (const { cell, index, program, built } of chunk) {
      const value = results.get(built.id);
      const site = siteOf(built);
      const verdict = value?.error ? { outcome: "harness", detail: { why: value.error } } : judge({ code: cell.code, site, typecheck: value.typecheck, checker: value.checker, runtime: value.runtime });
      const misuse = program.components[0].misuse;
      rows.push({ id: built.id, seed: program.seed, index, code: cell.code, context: cell.context, variant: misuse.variant, binding: misuse.binding, layer: misuse.layer, site, outcome: verdict.outcome, detail: verdict.detail, lines: built.text.split("\n").length });
      values.set(built.id, value);
    }
    console.error(`matrix: ${Math.min(at + CHUNK, entries.length)}/${entries.length} programs, ${Math.round((Date.now() - started) / 1000)}s, ${JSON.stringify(tally(rows))}`);
  }

  assertFindingsMapped();
  const refusal = typeErrorRefusal(rows.filter((row) => row.outcome === "typed-away").length, rows.length, { first: rows.find((row) => row.outcome === "typed-away")?.detail?.errors?.[0] });
  if (refusal) throw new Error(refusal);

  // Chrome: a sample of agree programs per cell, every wrong-rule and false-clean one.
  if (options.confirmWithChrome) {
    const pick = [];
    const bySample = new Map();
    // Per cell: up to --chrome-sample agree programs, up to --chrome-false-clean false-clean programs (the smallest first: they are the
    // ones the shrinker takes), every wrong-rule program. Every cell with a false clean or a wrong rule is thereby confirmed in Chrome.
    const limits = { agree: options.chromeSample ?? 3, "false-clean": options.chromeFalseClean ?? 4, "wrong-rule": Infinity };
    for (const row of [...rows].sort((a, b) => (a.binding === "destructured") - (b.binding === "destructured") || a.lines - b.lines || a.index - b.index)) {
      if (!(row.outcome in limits)) continue;
      const key = `${row.code}|${row.context}|${row.outcome}`;
      const n = bySample.get(key) ?? 0;
      if (n < limits[row.outcome]) { bySample.set(key, n + 1); pick.push(row); }
    }
    console.error(`matrix: Chrome confirmation of ${pick.length} programs`);
    for (let at = 0; at < pick.length; at += 40) {
      const chunk = pick.slice(at, at + 40);
      const records = await chromeRecords(chunk.map((row) => values.get(row.id)), config, options, out);
      for (const row of chunk) row.chrome = judgeChrome(row, records.get(row.id));
      // A browser that would not close in time or a clean run that did not report is the machine's, not the program's: once more.
      const retry = chunk.filter((row) => row.chrome.status === "chrome-inconclusive" && /browser close|no clean reference|no record/.test(String(row.chrome.why)));
      if (retry.length) {
        const again = await chromeRecords(retry.map((row) => values.get(row.id)), config, options, out);
        for (const row of retry) { const next = judgeChrome(row, again.get(row.id)); if (next.status !== "chrome-inconclusive") row.chrome = { ...next, retried: true }; }
      }
      console.error(`matrix: Chrome ${Math.min(at + 40, pick.length)}/${pick.length}`);
    }
  }

  // Shrink one representative per false-clean cell (the smallest program that Chrome did not refute).
  if (options.shrink) {
    const cellsWithFalseClean = new Map();
    // A destructured binding is the commonest cause of a false clean by far; a cell's representative avoids it when the cell has another
    // (then the minimum shows the next cause), and otherwise is the smallest program.
    const smaller = (a, b) => (a.binding === "destructured") !== (b.binding === "destructured") ? b.binding === "destructured" : a.lines < b.lines;
    for (const row of rows.filter((r) => r.outcome === "false-clean" && (!r.chrome || r.chrome.status === "same"))) {
      const key = `${row.code}|${row.context}`;
      const best = cellsWithFalseClean.get(key);
      if (!best || smaller(row, best)) cellsWithFalseClean.set(key, row);
    }
    for (const [key, row] of cellsWithFalseClean) {
      try {
        const minimal = await shrinkRepresentative(row, values.get(row.id), config, options, out);
        let chrome = null;
        if (options.confirmWithChrome) {
          const result = (await evaluate([{ program: minimal.program, built: minimal.built }], { ...config, casesDir: join(out, "shrink") })).get(minimal.built.id);
          const record = (await chromeRecords([result], config, options, out)).get(minimal.built.id);
          chrome = judgeChrome({ code: row.code, site: siteOf(minimal.built), outcome: "false-clean" }, record).status;
        }
        row.minimal = { repro: minimal.dir.replace(`${out}/`, ""), size: minimal.size, reductions: minimal.reductions, runs: minimal.runs, chrome };
        console.error(`matrix: shrunk ${key}: ${minimal.size} lines (${minimal.reductions} reductions)${chrome ? `, Chrome ${chrome}` : ""}`);
      } catch (error) {
        console.error(`matrix: shrink ${key} failed: ${error.message}`);
      }
    }
  }

  // Typed-away codes: the evidence is the type checker's own error on the one-line misuse.
  const typedAway = {};
  for (const code of MISUSE_CODES.filter((c) => SPEC[c].typedAway)) {
    const dir = materializeForChecker({ files: { "App.tsx": SPEC[code].probe } }, config.install, `typed-away-${code}`);
    const result = await typecheck(dir, { tsc: config.tsc });
    rmSync(dir, { recursive: true, force: true });
    typedAway[code] = { rejected: !result.ok, error: result.errors?.[0] ?? null };
  }

  const header = { label: options.checkerLabel ?? null, bin: options.checkerBin ? "release build" : null };
  const matrix = buildMatrix(rows, { options, header, typedAway });
  const statuses = codeStatuses(matrix);
  writeFileSync(join(out, "programs.jsonl"), rows.map((row) => JSON.stringify({ ...row, site: undefined, region: regionOf(row.site) })).join("\n") + "\n");
  writeFileSync(join(out, "matrix.json"), `${JSON.stringify({ ...matrix, statuses }, null, 2)}\n`);
  writeFileSync(join(out, "matrix.md"), markdown(matrix, statuses));
  console.error(`matrix: wrote ${join(out, "matrix.md")}`);
  if (options.baseline) return reportGate(gate(JSON.parse(readFileSync(resolve(options.baseline), "utf8")), JSON.parse(JSON.stringify(matrix))));
  return 0;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const argv = process.argv.slice(2);
    if (argv[0] === "--gate") {
      if (argv.length !== 3) throw new Error(`--gate needs <baseline.json> <current.json>\n${usage}`);
      process.exitCode = reportGate(gate(JSON.parse(readFileSync(resolve(argv[1]), "utf8")), JSON.parse(readFileSync(resolve(argv[2]), "utf8"))));
    } else {
      const options = parseArgs(argv, { extraValues: ["per-cell", "codes", "contexts", "chrome-sample", "chrome-false-clean", "baseline", "checker-label"] });
      for (const key of ["perCell", "chromeSample", "chromeFalseClean"]) if (options[key] !== undefined) options[key] = Number(options[key]);
      options.perCell ??= 20;
      if (options.help || !options.out) { console.error(usage); process.exitCode = options.help ? 0 : 2; }
      else process.exitCode = await runMatrix(options);
    }
  } catch (error) {
    console.error(`matrix: ${error.message}`);
    process.exitCode = 2;
  }
}
