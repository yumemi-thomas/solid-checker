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
import { browserExecutionCache, withRuntimeCache } from "./cache.mjs";
import { codeRules, codesForRule, ignoredRules, policyOf, withholdingRules } from "./codes.mjs";
import { runtimeEvents } from "./classify.mjs";
import { DEFAULT_ORACLE_INSTALL, runChrome } from "./chrome.mjs";
import { buildCase, writeCase } from "./emit.mjs";
import { GENERATOR_VERSION, generateProgram } from "./generate.mjs";
import { LAYERS, MISUSE_CODES, MOUNTS, SPEC, TEMPLATE_CODES, gridBindingsOf, gridVariantsOf, sourcesOf, sourcesOption } from "./misuse.mjs";
import { isSite, walk } from "./model.mjs";
import { evaluate, finishEntries, formatPhaseTimings, prepareEntries, runtimeCached } from "./pipeline.mjs";
import { programSeed } from "./rng.mjs";
import { shrink } from "./shrink.mjs";
import { legalTwin } from "./falseclean.mjs";
import { attachDaemon, configFrom, parseArgs } from "./run.mjs";
import { assertFindingsMapped, materializeForChecker } from "./checker.mjs";
import { TYPE_ERROR_REFUSAL_SHARE, harnessRefusal, requireTypeChecker, resolveTsc, typeErrorRefusal, typecheck } from "./typecheck.mjs";

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
export function judge({ code, site, typecheck: typed, checker, runtime, others = [] }) {
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
  // A combination program's other uses are legal sites of other codes: a finding on their lines is theirs, never this site's.
  const elsewhere = (finding) => others.some((lines) => inLines(finding, lines)) && !inLines(finding, region);
  const relevant = checker.findings.filter((finding) => !ignoredRules.includes(finding.rule) && !elsewhere(finding));
  const claims = (finding) => rules.includes(finding.rule) || withholdingRules.includes(finding.rule);
  const covering = relevant.filter((finding) => inLines(finding, region));
  // The checker anchors a finding at the call that runs the misuse (a closure's call, a helper's call), not always at the misuse
  // line itself. In a one-site program a claiming finding anywhere is this misuse's: it is an agree (or a gap) with `remote` set.
  const claimingViolation = relevant.find((finding) => finding.kind === "violation" && rules.includes(finding.rule) && inLines(finding, region))
    ?? relevant.find((finding) => finding.kind === "violation" && rules.includes(finding.rule));
  const remote = Boolean(claimingViolation) && !inLines(claimingViolation, region);
  const violations = covering.filter((finding) => finding.kind === "violation");
  const stray = relevant.filter((finding) => finding.kind === "violation" && !rules.includes(finding.rule) && !inLines(finding, region));
  // Another claimed code Chrome reports on the site's lines (a store link written in a body is a strict read too): the checker
  // missed it when no finding of its rules is in the file (`false-clean`), or declined it (`recall-gap`). The target's outcome
  // does not see it, so it is counted on its own.
  const secondary = Object.fromEntries([...new Set(known
    .filter((event) => event.code !== code && (codeRules[event.code] ?? []).length && attributed(event).some((position) => region.includes(position.line)))
    .map((event) => event.code))]
    .map((other) => [other, relevant.some((finding) => finding.kind === "violation" && codeRules[other].includes(finding.rule))
      ? "agree"
      : relevant.some((finding) => codeRules[other].includes(finding.rule)) ? "recall-gap" : "false-clean"])
    .filter(([, verdict]) => verdict !== "agree"));
  const atOthers = checker.findings.filter((finding) => finding.kind === "violation" && elsewhere(finding)).map((finding) => `${finding.rule}@${finding.line}`);
  const detail = { ...(atOthers.length ? { atOthers } : {}), placed, findings: covering.map((finding) => `${finding.rule}/${finding.kind}@${finding.line}`), ...(remote ? { remote: `${claimingViolation.rule}@${claimingViolation.line}` } : {}), ...(stray.length ? { stray: stray.map((finding) => `${finding.rule}@${finding.line}`) } : {}), ...(Object.keys(secondary).length ? { secondary } : {}) };
  if (claimingViolation) return { outcome: "agree", detail };
  // A violation of another rule that the runtime substantiates on these lines (one of that rule's codes reported here: a store read
  // in a body is a strict read too) is a second defect at the site, not a wrong rule; the code is judged as if it were absent.
  const corroborated = (finding) => codesForRule(finding.rule).some((other) => known.some((event) => event.code === other && attributed(event).some((position) => region.includes(position.line))));
  const wrong = violations.filter((finding) => !corroborated(finding));
  if (violations.length > wrong.length) detail.corroborated = [...new Set(violations.filter(corroborated).map((finding) => finding.rule))];
  if (wrong.length) return { outcome: "wrong-rule", detail: { ...detail, rules: [...new Set(wrong.map((finding) => finding.rule))] } };
  // A recall gap is the checker declining this code: an uncertifiable finding of a rule that claims it (or a withholding) at the site.
  // Another rule's uncertifiable finding on the same lines says nothing about this code; it is listed, and the program falls through.
  if (covering.some(claims)) return { outcome: "recall-gap", detail };
  const unrelated = covering.length ? { unrelated: covering.map((finding) => `${finding.rule}/${finding.kind}@${finding.line}`) } : {};
  const claimingElsewhere = relevant.find((finding) => claims(finding));
  if (claimingElsewhere) return { outcome: "recall-gap", detail: { ...detail, ...unrelated, remote: `${claimingElsewhere.rule}/${claimingElsewhere.kind}@${claimingElsewhere.line}` } };
  // Certified: the file has no uncertifiable finding at all, so the checker claims it proved the whole file. A false clean there is a
  // fail-closed break (invariant 2); in a file with an uncertifiable finding elsewhere it is a recall gap that only lost its place.
  const certified = !checker.findings.some((finding) => !ignoredRules.includes(finding.rule) && finding.kind === "uncertifiable");
  return { outcome: "false-clean", detail: { ...detail, ...unrelated, certified } };
}

/** The misuse site of a built case: its only `use` site. */
export const siteOf = (built) => built.sites.find((site) => site.primary) ?? built.sites.find((site) => site.code) ?? built.sites[0];
/** The regions of a combination program's other uses (misuse.mjs `buildCombo`): a finding there is about them, not the cell's site. */
export const otherRegions = (built) => built.sites.filter((site) => site.code && !site.primary && built.sites.some((other) => other.primary)).map(regionOf);

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

/**
 * `--grid`: every shape of a cell, one program each. A shape is the variant, the binding and the layer; on the recorded matrices every
 * program of one shape had the same outcome, and no smaller key did, so one program per shape judges what the random draw judges and
 * reaches the shapes twenty random draws miss. In a fixed order: variants, then bindings, then layers.
 */
export const gridShapes = (code, context, mounts = ["root"], sources = ["local"]) => gridVariantsOf(code)
  .filter((variant) => context === undefined || !SPEC[code].variantContexts?.[variant] || SPEC[code].variantContexts[variant].includes(context))
  .flatMap((variant) => gridBindingsOf(code, variant).flatMap((binding) => LAYERS.flatMap((layer) => shapeAxes(code, variant, binding, mounts, sources, context).map((axes) => ({ variant, binding, layer, ...axes })))));

/**
 * The mount and source of a shape's programs: each mount with the local source, then each other source the (code, variant,
 * binding) takes, mounted as the source says (misuse.mjs SOURCES; a source is not crossed with the mounts). `{}` is the root.
 */
export function shapeAxes(code, variant, binding, mounts, sources, context) {
  const takes = sourcesOf(code, variant, binding, context);
  return [
    ...(sources.includes("local") ? mounts.map((mount) => (mount === "root" ? {} : { mount })) : []),
    ...sources.filter((source) => source !== "local" && takes.includes(source)).map((source) => ({ source })),
  ];
}

/** `--mounts a,b` (misuse.mjs MOUNTS), default the entry's own render only. */
export function mountsOption(value) {
  const mounts = (value ?? "root").split(",");
  for (const mount of mounts) if (!MOUNTS.includes(mount)) throw new Error(`--mounts: unknown mount ${mount} (${MOUNTS.join(", ")})`);
  return mounts;
}

/** The id suffix of a shape's mount (none for the entry's own render, so a root program keeps its id). */
export const mountSuffix = (shape) => `${shape.mount ? `-m-${shape.mount}` : ""}${shape.source ? `-s-${shape.source}` : ""}${shape.combo ? "-c" : ""}`;
/** The seed key's tail of a shape's mount and source (none for a root, local program, so its seed stays). */
export const axesKey = (shape) => `${shape.mount ? `/${shape.mount}` : ""}${shape.source ? `/s:${shape.source}` : ""}${shape.combo ? "/combo" : ""}`;
/** `--combine`: each shape's program is a combination program (misuse.mjs `buildCombo`) instead of the shape's own. */
export const combined = (shapes, combine) => (combine ? shapes.map((shape) => ({ ...shape, combo: true })) : shapes);

/** A shape's program seed depends on the run seed and the shape only, so a shape's program does not move when the grid grows. */
export const shapeSeed = (base, code, context, shape) => programSeed((Number(base) ^ fnv(`${code}/${context}/${shape.variant ?? "-"}/${shape.binding}/${shape.layer}${axesKey(shape)}`)) >>> 0, 0);

export function generateShape(options, { code, context }, shape, index) {
  const seed = shapeSeed(options.seed, code, context, shape);
  const program = generateProgram(seed, { profile: "misuse", misusePlan: { code, context, ...shape } });
  const id = `mx-${code.toLowerCase().replaceAll("_", "-")}-${context}-${shape.variant ?? "x"}-${shape.binding}-${shape.layer.replace("+", "-")}${mountSuffix(shape)}`;
  return { program, built: buildCase(program, { id }), index };
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
  const chromeOptions = { out, install: config.install, bin: config.bin, typefacts: config.typefacts, browser: options.browser, concurrency: options.concurrency, executionCache: browserExecutionCache(options) };
  const cases = values.map((value) => ({ id: value.built.id, dir: value.caseDir, sites: value.built.sites }));
  return runChrome(cases, { ...chromeOptions, withClean: true });
}

/** Chrome's outcome for a program, and whether it equals the fast path's. */
function judgeChrome(row, record) {
  if (!record || record.runtime?.failure || !record.checker?.ok || record.checker.perturbed?.length) return { status: "chrome-inconclusive", why: record?.runtime?.failure?.message ?? record?.checker?.error ?? record?.checker?.perturbed?.[0] ?? "no record" };
  const { outcome, detail } = judge({ code: row.code, site: row.site, typecheck: { ok: true }, checker: record.checker, runtime: record.runtime, others: row.others });
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
    const verdict = judge({ code: target.code, site, typecheck: result.typecheck, checker: result.checker, runtime: result.runtime, others: otherRegions(built) });
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

/**
 * Outcomes by binding x layer across every code: one root cause in the checker (a value alias lost, a helper not followed) shows once
 * here, where the cells count it once per code.
 */
export function featureTable(rows) {
  const out = {};
  for (const row of rows) {
    const key = `${row.binding}|${row.layer}`;
    const slot = (out[key] ??= { programs: 0, ...emptyTally(), falseCleanCertified: 0, codes: [] });
    slot.programs += 1;
    slot[row.outcome] += 1;
    if (row.outcome === "false-clean" && row.detail?.certified) slot.falseCleanCertified += 1;
    if (row.outcome === "false-clean" && !slot.codes.includes(row.code)) slot.codes.push(row.code);
  }
  return Object.fromEntries(Object.entries(out).sort(([a], [b]) => (a < b ? -1 : 1)).map(([key, slot]) => [key, { ...slot, codes: slot.codes.sort() }]));
}

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
      falseCleanCertified: cell.rows.filter((row) => row.outcome === "false-clean" && row.detail?.certified).length,
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
  // Second codes Chrome reported at a site that the checker missed or declined, per code and verdict.
  const secondary = {};
  for (const row of rows) for (const [other, verdict] of Object.entries(row.detail?.secondary ?? {})) {
    const entry = (secondary[other] ??= { "false-clean": [], "recall-gap": [] });
    entry[verdict].push(row.id);
  }
  return {
    strays: Object.fromEntries(Object.entries(strays).map(([rule, ids]) => [rule, { programs: ids.length, examples: ids.slice(0, 3) }])),
    secondary: Object.fromEntries(Object.entries(secondary).map(([other, entry]) => [other, Object.fromEntries(Object.entries(entry).map(([verdict, ids]) => [verdict, { programs: ids.length, examples: ids.slice(0, 3) }]))])),
    generator: GENERATOR_VERSION, profile: "misuse", seed: options.seed, perCell: options.grid ? null : options.perCell, ...(options.grid ? { sampling: "grid" } : {}), checker: header, total,
    confirmed: CONFIRMED.reduce((sum, name) => sum + total[name], 0),
    falseCleanCertified: rows.filter((row) => row.outcome === "false-clean" && row.detail?.certified).length,
    byFeature: featureTable(rows), typedAway, cells: out,
  };
}

/**
 * How many checker runs had their event-handler proofs withheld by the harness's own entry (checker.mjs `entryVeto`): with any, a
 * handler context's outcomes say nothing about the checker's handler proofs. Empty when none was.
 */
export function entryVetoSummary(values) {
  const vetoed = [...values.values()].map((value) => value?.checker?.entryVeto).filter(Boolean);
  if (!vetoed.length) return {};
  console.error(`WARNING: the harness entry withheld event-handler proofs in ${vetoed.length} checker run(s): ${vetoed[0]}`);
  return { entryVeto: { runs: vetoed.length, premise: vetoed[0] } };
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
  lines.push(`Generator ${matrix.generator}, seed ${matrix.seed}, ${matrix.sampling === "grid" ? "one program per shape (variant, binding, layer)" : `${matrix.perCell} programs per cell`}. Checker: ${matrix.checker.label ?? "?"}${matrix.checker.bin ? ` (${matrix.checker.bin})` : ""}.`, "");
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
  lines.push("", `Totals: ${Object.entries(t).map(([k, v]) => `${k} ${v}`).join(", ")}; runtime-confirmed ${matrix.confirmed}. False cleans in a certified file (no uncertifiable finding anywhere: a fail-closed break): ${matrix.falseCleanCertified ?? "-"}.`, "");
  if (matrix.byFeature) {
    lines.push("## By binding and layer, across codes", "", "| binding | layer | programs | agree | recall-gap | false-clean | certified false-clean | codes with a false clean |", "| --- | --- | --- | --- | --- | --- | --- | --- |");
    for (const [key, slot] of Object.entries(matrix.byFeature)) {
      const [binding, layer] = key.split("|");
      lines.push(`| ${binding} | ${layer} | ${slot.programs} | ${slot.agree} | ${slot["recall-gap"]} | ${slot["false-clean"]} | ${slot.falseCleanCertified} | ${slot.codes.length} |`);
    }
    lines.push("");
  }
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
 * A baseline cell of a code that is no longer a claimed defect (advisory now) is `retired`: reported, not failed.
 * @returns {{ ok: boolean, refused?: string, regressions: object[], improvements: object[], baselineUnjudged: object[], infrastructure: object[], retired: object[] }}
 */
export function gate(baseline, current) {
  const empty = { regressions: [], improvements: [], baselineUnjudged: [], infrastructure: [] };
  for (const field of ["seed", "perCell", "generator", "sampling"]) {
    // A matrix recorded before `sampling` existed drew at random.
    if ((field === "sampling" ? baseline[field] ?? "random" : baseline[field]) !== (field === "sampling" ? current[field] ?? "random" : current[field])) return { ok: false, refused: `the matrices differ in ${field} (${baseline[field]} vs ${current[field]}): the gate compares the same programs`, ...empty };
  }
  const now = current.total ?? {};
  const all = Object.values(now).reduce((sum, n) => sum + n, 0);
  const unjudged = (now["typed-away"] ?? 0) + (now.harness ?? 0);
  if (all && unjudged / all > TYPE_ERROR_REFUSAL_SHARE) return { ok: false, refused: `${unjudged} of ${all} programs of the current run are typed-away or harness: it judged nothing (the limit is ${TYPE_ERROR_REFUSAL_SHARE * 100}%)`, ...empty };
  const regressions = []; const improvements = []; const baselineUnjudged = []; const infrastructure = []; const retired = []; const newPrograms = [];
  for (const [key, base] of Object.entries(baseline.cells)) {
    const cell = current.cells[key];
    // A code the policy table no longer counts as a defect (it became advisory) is not measured any more: reported, never a regression.
    if (!cell && policyOf(base.code) && policyOf(base.code).disposition !== "defect-claimed") { retired.push({ cell: key, why: `${base.code} is ${policyOf(base.code)?.disposition ?? "unknown"} now, not measured` }); continue; }
    if (!cell) { regressions.push({ cell: key, why: "cell missing from the current matrix" }); continue; }
    // The baseline's programs. A program only the current run has (a grid shape added since) is new, reported, never a regression.
    const seeds = Object.keys(base.verdicts);
    const added = Object.keys(cell.verdicts).filter((seed) => !(seed in base.verdicts));
    if (added.length) newPrograms.push({ cell: key, programs: added.length });
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
  return { ok: regressions.length === 0, regressions, improvements, baselineUnjudged, infrastructure, retired, newPrograms };
}

function reportGate(result) {
  if (result.refused) { console.error(`gate refused: ${result.refused}`); return 2; }
  for (const item of result.improvements) console.error(`gate: improved ${item.cell}: ${item.why}`);
  for (const item of result.retired ?? []) console.error(`gate: retired ${item.cell}: ${item.why}`);
  if (result.newPrograms?.length) console.error(`gate: ${result.newPrograms.reduce((n, item) => n + item.programs, 0)} program(s) in ${result.newPrograms.length} cell(s) are new since the baseline (not compared)`);
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
  [--confirm-with-chrome [--chrome-sample 3] [--chrome-false-clean 4] [--chrome-other 2] [--browser <chromium>]] [--shrink [--budget N]]
  [--grid [--layers a,b]]               one program per shape (variant x binding x layer) of every cell, instead of --per-cell random draws
  [--mounts root,child,show,for]        with --grid, also mount each program as a parent's child (misuse.mjs MOUNTS; default root)
  [--sources local,props,context,module,async]  with --grid, also take the values from a parent's props, a context, helpers.ts or an
                                        async parent (misuse.mjs SOURCES; axis or source names; default local)
  [--combine]                           with --grid, each shape's program is a combination: the site among one or two legal uses of
                                        other codes and unrelated reactive code, its values from a drawn source (misuse.mjs buildCombo)
  [--cache-dir <dir>] [--no-cache]      the runtime cache (type checks and fast-path records; default $TMPDIR/solid-checker-fuzz-cache)
  [--no-chrome-cache]                   run Chrome's browser half fresh (by default feedback run --execution-cache reuses it; the checker always reruns)
  [--baseline <matrix.json>]            fail when a judged program leaves agree or enters false-clean (same seeds); moves out of or into typed-away/harness are reported, not failed
  node scripts/fuzz/matrix.mjs --gate <baseline.json> <current.json>`;

const CHUNK = 150;

export async function runMatrix(options) {
  const out = resolve(options.out);
  if (existsSync(out)) throw new Error(`use a fresh --out path (${out} exists)`);
  requireTypeChecker(resolveTsc(options.tsc));
  mkdirSync(out, { recursive: true });
  const config = withRuntimeCache(attachDaemon(configFrom({ ...options, profile: "misuse" }, out), options), options);
  try {
    return await sweep(options, config, out);
  } finally {
    await config.fast?.close();
  }
}

async function sweep(options, config, out) {
  const cells = cellList({ codes: options.codes?.split(","), contexts: options.contexts?.split(",") });
  const entries = [];
  // `--layers a,b` keeps those layers' shapes (their index, and so their programs, as in the whole grid).
  const layerKept = (shape) => !options.layers || options.layers.split(",").includes(shape.layer);
  const mounts = mountsOption(options.mounts);
  const sources = sourcesOption(options.sources);
  if (options.grid) for (const cell of cells) combined(gridShapes(cell.code, cell.context, mounts, sources), options.combine).forEach((shape, index) => { if (layerKept(shape)) entries.push({ cell, ...generateShape(options, cell, shape, index) }); });
  else for (const cell of cells) for (let index = 0; index < options.perCell; index += 1) entries.push({ cell, index, ...generateCell(options, cell, index) });
  console.error(`matrix: ${cells.length} cells, ${entries.length} programs (${TEMPLATE_CODES.length} codes); evaluating`);
  const rows = []; const values = new Map();
  const started = Date.now();
  // With every program's runtime cached no Vitest batch runs, so the workers' share of the machine goes to the checker: the same budget.
  const plain = (list) => list.map(({ program, built }) => ({ program, built }));
  const run = runtimeCached(plain(entries), config) ? { ...config, concurrency: config.concurrency + config.workers } : config;
  if (run !== config) console.error(`matrix: every program's runtime is cached; checker concurrency ${run.concurrency} (${config.concurrency} + the ${config.workers} idle workers)`);
  // Chunk k's fast path runs while chunk k+1 is type-checked and checked: one checker pool and one Vitest batch at a time, as before, but
  // never one waiting on the other. Each chunk's records and verdicts are what `evaluate` gives it.
  const chunks = [];
  for (let at = 0; at < entries.length; at += CHUNK) chunks.push(entries.slice(at, at + CHUNK));
  const prepare = (chunk) => { const stage = prepareEntries(plain(chunk), run); stage.catch(() => {}); return stage; };
  let next = chunks.length ? prepare(chunks[0]) : null;
  for (const [number, chunk] of chunks.entries()) {
    const stage = await next;
    next = number + 1 < chunks.length ? prepare(chunks[number + 1]) : null;
    const results = await finishEntries(stage, run);
    const at = number * CHUNK;
    for (const { cell, index, program, built } of chunk) {
      const value = results.get(built.id);
      const site = siteOf(built);
      const verdict = value?.error ? { outcome: "harness", detail: { why: value.error } } : judge({ code: cell.code, site, typecheck: value.typecheck, checker: value.checker, runtime: value.runtime, others: otherRegions(built) });
      const misuse = program.components[0].misuse;
      const others = otherRegions(built);
      rows.push({ id: built.id, seed: program.seed, index, code: cell.code, context: cell.context, variant: misuse.variant, binding: misuse.binding, layer: misuse.layer, ...(misuse.mount && misuse.mount !== "root" ? { mount: misuse.mount } : {}), ...(misuse.source ? { source: misuse.source } : {}), ...(misuse.combo ? { combo: misuse.combo, others } : {}), site, outcome: verdict.outcome, detail: verdict.detail, lines: built.text.split("\n").length });
      values.set(built.id, value);
    }
    console.error(`matrix: ${Math.min(at + CHUNK, entries.length)}/${entries.length} programs, ${Math.round((Date.now() - started) / 1000)}s, ${JSON.stringify(tally(rows))}`);
  }

  console.error(`matrix: evaluated in ${Math.round((Date.now() - started) / 1000)}s; busy time ${formatPhaseTimings()}`);
  assertFindingsMapped();
  const refusal = typeErrorRefusal(rows.filter((row) => row.outcome === "typed-away").length, rows.length, { first: rows.find((row) => row.outcome === "typed-away")?.detail?.errors?.[0] });
  if (refusal) throw new Error(refusal);
  const broken = harnessRefusal(rows.filter((row) => row.outcome === "harness").length, rows.length, { first: rows.find((row) => row.outcome === "harness")?.detail?.why });
  if (broken) throw new Error(broken);

  // Chrome: a sample of agree programs per cell, every wrong-rule and false-clean one.
  if (options.confirmWithChrome) {
    const pick = [];
    const bySample = new Map();
    // Per cell: up to --chrome-sample agree programs, up to --chrome-false-clean false-clean programs (the smallest first: they are the
    // ones the shrinker takes), every wrong-rule program, and up to --chrome-other recall-gap and not-fired programs (a not-fired program
    // Chrome fires is a Happy DOM gap; a recall gap Chrome does not reproduce is a fast-path artefact). Every cell with a false clean or
    // a wrong rule is thereby confirmed in Chrome.
    const other = options.chromeOther ?? 2;
    const limits = { agree: options.chromeSample ?? 3, "false-clean": options.chromeFalseClean ?? 4, "wrong-rule": Infinity, "recall-gap": other, "not-fired": other };
    for (const row of [...rows].sort((a, b) => (a.binding === "destructured") - (b.binding === "destructured") || a.lines - b.lines || a.index - b.index)) {
      if (!(row.outcome in limits)) continue;
      const key = `${row.code}|${row.context}|${row.outcome}`;
      const n = bySample.get(key) ?? 0;
      if (n < limits[row.outcome]) { bySample.set(key, n + 1); pick.push(row); }
    }
    console.error(`matrix: Chrome confirmation of ${pick.length} programs`);
    const chromeStarted = Date.now();
    // Chunks only pace the progress line and the retries: the oracle's pool runs a chunk's cases independently, so a larger chunk
    // changes no record, and spends less time waiting on one chunk's slowest case before the next starts.
    const CHROME_CHUNK = 160;
    for (let at = 0; at < pick.length; at += CHROME_CHUNK) {
      const chunk = pick.slice(at, at + CHROME_CHUNK);
      const records = await chromeRecords(chunk.map((row) => values.get(row.id)), config, options, out);
      for (const row of chunk) row.chrome = judgeChrome(row, records.get(row.id));
      // A browser that would not close in time or a clean run that did not report is the machine's, not the program's: once more.
      const retry = chunk.filter((row) => row.chrome.status === "chrome-inconclusive" && /browser close|no clean reference|no record/.test(String(row.chrome.why)));
      if (retry.length) {
        const again = await chromeRecords(retry.map((row) => values.get(row.id)), config, options, out);
        for (const row of retry) { const next = judgeChrome(row, again.get(row.id)); if (next.status !== "chrome-inconclusive") row.chrome = { ...next, retried: true }; }
      }
      console.error(`matrix: Chrome ${Math.min(at + CHROME_CHUNK, pick.length)}/${pick.length}, ${Math.round((Date.now() - chromeStarted) / 1000)}s`);
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
  const matrix = { ...buildMatrix(rows, { options, header, typedAway }), ...entryVetoSummary(values) };
  const statuses = codeStatuses(matrix);
  writeFileSync(join(out, "programs.jsonl"), rows.map((row) => JSON.stringify({ ...row, site: undefined, region: regionOf(row.site) })).join("\n") + "\n");
  writeFileSync(join(out, "matrix.json"), `${JSON.stringify({ ...matrix, statuses }, null, 2)}\n`);
  writeFileSync(join(out, "matrix.md"), markdown(matrix, statuses));
  if (config.runtimeCache) {
    // Beside the matrix, not in it: a cached run's matrix.json is the uncached run's, byte for byte.
    writeFileSync(join(out, "cache.json"), `${JSON.stringify(config.runtimeCache.stats(), null, 2)}\n`);
    console.error(`matrix: runtime cache ${JSON.stringify(config.runtimeCache.stats())}`);
  }
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
      const options = parseArgs(argv, { extraValues: ["per-cell", "codes", "contexts", "chrome-sample", "chrome-false-clean", "chrome-other", "layers", "baseline", "checker-label", "cache-dir", "mounts", "sources"], extraFlags: ["grid", "no-chrome-cache", "combine"] });
      for (const key of ["perCell", "chromeSample", "chromeFalseClean", "chromeOther"]) if (options[key] !== undefined) options[key] = Number(options[key]);
      options.perCell ??= 20;
      if (options.help || !options.out) { console.error(usage); process.exitCode = options.help ? 0 : 2; }
      else process.exitCode = await runMatrix(options);
    }
  } catch (error) {
    console.error(`matrix: ${error.message}`);
    process.exitCode = 2;
  }
}
