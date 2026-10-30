#!/usr/bin/env node
// The legal-context matrix: the misuse matrix's twin for false positives. For every code with `legal` contexts (misuse.mjs), the
// same call placed where it is correct Solid (a write in a handler, `onCleanup` in a component body, a strict read in a memo, a store
// write through its draft or inside its own setter), judged at its site:
//
//   typed-away        tsc rejects the program (not evidence)
//   harness           the checker or the runtime run failed (not evidence)
//   not-legal         the runtime reports the code anyway: the template is not legal there, a generator problem to fix
//   disturbed         the program threw, halted or could not be driven, and the checker reports a violation there: not judgeable
//   false-positive    a violation covers the site, the hit ledger shows the site ran, and the runtime reports none of the rule's
//                     codes: a candidate false positive (Chrome confirms; a confirmed one exits 3, stop and report)
//   unexecuted        a violation covers a site that never ran (ADR 0458: such a violation is unproven; reported, not gated)
//   uncertifiable     the checker declined at the site
//   clean             no finding at the site
//
//   node scripts/fuzz/legal.mjs --out <fresh dir> [--seed N] [--per-cell N | --grid] [--codes A,B] [--confirm-with-chrome]
//     [--checker-bin …] [--typefacts …] [--tsc …] [--cache-dir <dir>] [--no-cache]
import { existsSync, mkdirSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { browserExecutionCache, withRuntimeCache } from "./cache.mjs";
import { runChrome } from "./chrome.mjs";
import { entryVetoSummary } from "./matrix.mjs";
import { assertFindingsMapped } from "./checker.mjs";
import { disturbances, runtimeEvents } from "./classify.mjs";
import { codesForRule, ignoredRules, ruleHasRuntimeCounterpart } from "./codes.mjs";
import { buildCase } from "./emit.mjs";
import { generateProgram } from "./generate.mjs";
import { hitCount } from "./hits.mjs";
import { fnv, regionOf, runtimeDifferential, siteOf } from "./matrix.mjs";
import { parseAxes, parsePerStratum, stratumOf } from "./runtime-diff.mjs";
import { LAYERS, LEGAL_CODES, SPEC, gridBindingsOf, legalVariantsOf } from "./misuse.mjs";
import { axesKey, combined, mountSuffix, mountsOption, shapeAxes } from "./matrix.mjs";
import { sourcesOption } from "./misuse.mjs";
import { finishEntries, formatPhaseTimings, prepareEntries, runtimeCached } from "./pipeline.mjs";
import { programSeed } from "./rng.mjs";
import { attachDaemon, configFrom, parseArgs } from "./run.mjs";
import { keepIds } from "./smoke.mjs";
import { harnessRefusal, requireTypeChecker, resolveTsc, typeErrorRefusal } from "./typecheck.mjs";

export const LEGAL_OUTCOMES = ["typed-away", "harness", "not-legal", "disturbed", "false-positive", "unexecuted", "uncertifiable", "clean"];

const inLines = (finding, lines) => lines.some((line) => finding.line <= line && line <= finding.endLine);

/** Rules whose violation claims the site ran: an unexecuted site contradicts it. */
const RUN_CLAIMING_RULES = ["pending-async-unsuspendable-read"];

/**
 * Judges one legal program at its site.
 * @param {{ code: string, site: object, typecheck: object, checker: object, runtime: object, plan?: object }} input
 */
export function judgeLegal({ code, site, typecheck: typed, checker, runtime, plan, atSite: siteScoped = false }) {
  if (typed && typed.ok === false) return { outcome: "typed-away", detail: { error: typed.errors?.[0] } };
  if (!checker?.ok) return { outcome: "harness", detail: { why: `checker: ${String(checker?.error ?? "not run").slice(0, 120)}` } };
  if (!runtime) return { outcome: "harness", detail: { why: "no runtime record" } };
  const events = runtimeEvents(runtime);
  // A program whose runtime failed before reporting anything (a module that would not load) says nothing about legality.
  if (runtime.failure && !events.length && !(hitCount(runtime.hits ?? null, site.hit) > 0)) return { outcome: "harness", detail: { why: `runtime: ${String(runtime.failure.message).slice(0, 120)}` } };
  const region = regionOf(site);
  const relevant = checker.findings.filter((finding) => !ignoredRules.includes(finding.rule));
  const violations = relevant.filter((finding) => finding.kind === "violation");
  const atSite = violations.filter((finding) => inLines(finding, region));
  const stray = violations.filter((finding) => !inLines(finding, region)).map((finding) => `${finding.rule}@${finding.line}`);
  const detail = { findings: relevant.filter((finding) => inLines(finding, region)).map((finding) => `${finding.rule}/${finding.kind}@${finding.line}`), ...(stray.length ? { stray } : {}) };
  // `atSite` (a combination program): only a diagnostic on the use's own lines says that use is not legal; another use of the file
  // may provoke the same code on purpose (a `-guarded-rerun` read is a strict read Solid reports).
  const onSite = (event) => !siteScoped || event.positions.some((position) => (position.file === "App.tsx" || position.file === undefined) && region.includes(position.line));
  if (events.some((event) => event.code === code && onSite(event))) return { outcome: "not-legal", detail: { ...detail, reported: [...new Set(events.map((event) => event.code))] } };
  // A run-claiming violation anywhere (a call attributed the read in a closure) whose codes the runtime never reports (ADR 0504 review).
  const claimedRuns = violations.filter((finding) => RUN_CLAIMING_RULES.includes(finding.rule) && !events.some((event) => codesForRule(finding.rule).includes(event.code)));
  if (claimedRuns.length && !disturbances(runtime, plan ?? { clicks: [] }).length) return { outcome: "false-positive", detail: { ...detail, rules: [...new Set(claimedRuns.map((finding) => finding.rule))], at: claimedRuns.map((finding) => finding.line) } };
  if (!atSite.length) {
    return relevant.some((finding) => finding.kind === "uncertifiable" && inLines(finding, region)) ? { outcome: "uncertifiable", detail } : { outcome: "clean", detail };
  }
  const disturbed = disturbances(runtime, plan ?? { clicks: [] });
  if (disturbed.length) return { outcome: "disturbed", detail: { ...detail, why: disturbed[0] } };
  // A violation of a rule the runtime can contradict, none of whose codes the runtime reports anywhere.
  const contradicted = atSite.filter((finding) => ruleHasRuntimeCounterpart(finding.rule) && !events.some((event) => codesForRule(finding.rule).includes(event.code)));
  if (!contradicted.length) return { outcome: "uncertifiable", detail: { ...detail, why: "a violation whose rule no runtime code contradicts, or whose code the runtime reports" } };
  const ran = hitCount(runtime.hits ?? null, site.hit);
  if (ran === null) return { outcome: "disturbed", detail: { ...detail, why: "no hit ledger" } };
  // A rule whose violation says the site ran (SC5001: "is read in the first run") is wrong where it never ran (ADR 0504 review).
  if (ran === 0 && !contradicted.some((finding) => RUN_CLAIMING_RULES.includes(finding.rule))) return { outcome: "unexecuted", detail };
  return { outcome: "false-positive", detail: { ...detail, rules: [...new Set(contradicted.map((finding) => finding.rule))] } };
}

/** Outcomes from worst to best: a combination program takes its worst site's. */
const WORST_FIRST = ["false-positive", "not-legal", "disturbed", "harness", "typed-away", "unexecuted", "uncertifiable", "clean"];

/**
 * Judges a legal program: at its site, or, for a combination program (`--combine`, misuse.mjs `buildCombo`), at every use, each with
 * its own code (every use there is legal), taking the worst; `detail.sites` lists each use's outcome.
 */
export function judgeLegalProgram({ built, ...input }) {
  const uses = built.sites.filter((site) => site.code);
  if (!uses.some((site) => site.primary)) return judgeLegal({ ...input, site: siteOf(built) });
  const verdicts = uses.map((site) => ({ site, ...judgeLegal({ ...input, code: site.code, site, atSite: true }) }));
  const worst = verdicts.reduce((a, b) => (WORST_FIRST.indexOf(b.outcome) < WORST_FIRST.indexOf(a.outcome) ? b : a));
  return { outcome: worst.outcome, detail: { ...worst.detail, at: worst.site.line, code: worst.site.code, sites: verdicts.map((v) => `${v.site.code}@${v.site.line}:${v.outcome}`) } };
}

/** The cells: every code with legal contexts, each legal context. */
export function legalCells({ codes } = {}) {
  return LEGAL_CODES().filter((code) => !codes || codes.includes(code)).flatMap((code) => SPEC[code].legal.map((context) => ({ code, context })));
}

/** The legal shapes of a code (`--grid`): its legal variants x its grid bindings x the layers. */
export const legalShapes = (code, mounts = ["root"], sources = ["local"], context) => legalVariantsOf(code).flatMap((variant) => gridBindingsOf(code, variant).flatMap((binding) => LAYERS.flatMap((layer) => shapeAxes(code, variant, binding, mounts, sources, context).map((axes) => ({ variant, binding, layer, ...axes })))));

/** One legal program: its seed depends on the run seed and the cell and the index (random) or the shape (grid) only. */
export function generateLegal(options, { code, context }, { index, shape }) {
  const key = shape ? `legal/${code}/${context}/${shape.variant ?? "-"}/${shape.binding}/${shape.layer}${axesKey(shape)}` : `legal/${code}/${context}`;
  const seed = programSeed((Number(options.seed) ^ fnv(key)) >>> 0, shape ? 0 : index);
  const program = generateProgram(seed, { profile: "misuse", misusePlan: { code, context, legal: true, ...(shape ?? {}) } });
  const suffix = shape ? `${shape.variant ?? "x"}-${shape.binding}-${shape.layer.replace("+", "-")}${mountSuffix(shape)}` : String(index);
  return { program, built: buildCase(program, { id: `lg-${code.toLowerCase().replaceAll("_", "-")}-${context}-${suffix}` }) };
}

const tally = (rows) => Object.fromEntries(LEGAL_OUTCOMES.map((outcome) => [outcome, rows.filter((row) => row.outcome === outcome).length]));

export async function runLegal(options) {
  const out = resolve(options.out);
  if (existsSync(out)) throw new Error(`use a fresh --out path (${out} exists)`);
  requireTypeChecker(resolveTsc(options.tsc));
  mkdirSync(out, { recursive: true });
  const config = withRuntimeCache(attachDaemon(configFrom({ ...options, profile: "misuse" }, out), options), options);
  try {
    const cells = legalCells({ codes: options.codes?.split(",") });
    let entries = [];
    for (const cell of cells) {
      if (options.grid) combined(legalShapes(cell.code, mountsOption(options.mounts), sourcesOption(options.sources), cell.context), options.combine).filter((shape) => !options.layers || options.layers.split(",").includes(shape.layer)).forEach((shape, index) => entries.push({ cell, index, ...generateLegal(options, cell, { shape }) }));
      else for (let index = 0; index < options.perCell; index += 1) entries.push({ cell, index, ...generateLegal(options, cell, { index }) });
    }
    // `--ids <manifest>` (smoke.mjs): the programs it names, as the whole grid generates them.
    if (options.ids) entries = keepIds(entries, options.ids, "legal");
    console.error(`legal: ${cells.length} cells, ${entries.length} programs`);
    const rows = [];
    const values = new Map();
    const started = Date.now();
    // As matrix.mjs: with every runtime cached no Vitest batch runs, so the workers' share goes to the checker; and chunk k's fast
    // path runs while chunk k+1 is type-checked and checked. Each chunk's records and verdicts are what `evaluate` gives it.
    const plain = (list) => list.map(({ program, built }) => ({ program, built }));
    const run = runtimeCached(plain(entries), config) ? { ...config, concurrency: config.concurrency + config.workers } : config;
    if (run !== config) console.error(`legal: every program's runtime is cached; checker concurrency ${run.concurrency}`);
    const chunks = [];
    for (let at = 0; at < entries.length; at += 150) chunks.push(entries.slice(at, at + 150));
    const prepare = (chunk) => { const stage = prepareEntries(plain(chunk), run); stage.catch(() => {}); return stage; };
    let next = chunks.length ? prepare(chunks[0]) : null;
    for (const [number, chunk] of chunks.entries()) {
      const at = number * 150;
      const stage = await next;
      next = number + 1 < chunks.length ? prepare(chunks[number + 1]) : null;
      const results = await finishEntries(stage, run);
      for (const { cell, program, built } of chunk) {
        const value = results.get(built.id);
        const site = siteOf(built);
        const verdict = value?.error ? { outcome: "harness", detail: { why: value.error } } : judgeLegalProgram({ code: cell.code, built, typecheck: value.typecheck, checker: value.checker, runtime: value.runtime, plan: built.plan });
        const misuse = program.components[0].misuse;
        rows.push({ id: built.id, seed: program.seed, code: cell.code, context: cell.context, variant: misuse.variant, binding: misuse.binding, layer: misuse.layer, ...(misuse.mount && misuse.mount !== "root" ? { mount: misuse.mount } : {}), ...(misuse.source ? { source: misuse.source } : {}), ...(misuse.combo ? { combo: misuse.combo } : {}), outcome: verdict.outcome, detail: verdict.detail, site: { line: site.line, hit: site.hit, aux: site.aux } });
        values.set(built.id, value);
      }
      console.error(`legal: ${Math.min(at + 150, entries.length)}/${entries.length}, ${Math.round((Date.now() - started) / 1000)}s, ${JSON.stringify(tally(rows))}`);
    }
    console.error(`legal: evaluated in ${Math.round((Date.now() - started) / 1000)}s; busy time ${formatPhaseTimings()}`);
    assertFindingsMapped();
    const refusal = typeErrorRefusal(rows.filter((row) => row.outcome === "typed-away").length, rows.length);
    if (refusal) throw new Error(refusal);
    const broken = harnessRefusal(rows.filter((row) => row.outcome === "harness").length, rows.length, { first: rows.find((row) => row.outcome === "harness")?.detail?.why });
    if (broken) throw new Error(broken);
    // Chrome: every false-positive candidate, and up to --chrome-sample programs per cell of each other judged outcome (a clean program
    // Chrome reports the code for is an illegal "legal" context, the generator's bug; an unexecuted one Chrome runs is a Happy DOM gap).
    // Each twin runs with the hit ledger beside its clean control.
    let confirmed = 0;
    const candidates = rows.filter((row) => row.outcome === "false-positive");
    if (options.confirmWithChrome) {
      const sampled = new Map();
      const limit = options.chromeSample ?? 2;
      const pick = rows.filter((row) => {
        if (row.outcome === "false-positive") return true;
        if (!["clean", "uncertifiable", "unexecuted", "not-legal"].includes(row.outcome)) return false;
        const key = options.chromeStrata ? stratumOf(row, parseAxes(options.runtimeDiffAxes)) : `${row.code}|${row.context}|${row.outcome}`;
        const n = sampled.get(key) ?? 0;
        if (n >= limit) return false;
        sampled.set(key, n + 1);
        return true;
      });
      const cases = pick.map((row) => ({ id: row.id, dir: values.get(row.id).caseDir, sites: values.get(row.id).built.sites }));
      const chromeStarted = Date.now();
      const records = pick.length ? await runChrome(cases, { out, install: config.install, bin: config.bin, typefacts: config.typefacts, browser: options.browser, concurrency: options.chromeConcurrency ?? options.concurrency, withClean: true, executionCache: browserExecutionCache(options) }) : new Map();
      console.error(`legal: Chrome confirmed ${pick.length} programs in ${Math.round((Date.now() - chromeStarted) / 1000)}s`);
      for (const row of pick) {
        const value = values.get(row.id);
        const record = records.get(row.id);
        const judged = record?.checker?.ok && !record.checker.perturbed?.length ? judgeLegalProgram({ code: row.code, built: value.built, typecheck: { ok: true }, checker: record.checker, runtime: record.runtime, plan: value.built.plan }) : null;
        if (!judged) row.chrome = { status: "chrome-inconclusive", why: record?.checker?.perturbed?.[0] ?? record?.runtime?.failure?.message ?? "no record" };
        else if (row.outcome === "false-positive") row.chrome = { status: judged.outcome === "false-positive" ? "confirmed" : "not-confirmed", outcome: judged.outcome };
        else row.chrome = { status: judged.outcome === row.outcome ? "same" : "differs", outcome: judged.outcome };
        if (row.chrome.status === "confirmed") confirmed += 1;
      }
    }
    // The runtime differential (`--runtime-diff <N|all>`, matrix.mjs `runtimeDifferential`): Happy DOM's record against Chrome's.
    const perStratum = parsePerStratum(options.runtimeDiff);
    if (perStratum) await runtimeDifferential(rows, values, perStratum, config, options, out, "legal");
    const checked = rows.filter((row) => row.chrome);
    const chrome = checked.length ? Object.fromEntries(["confirmed", "not-confirmed", "same", "differs", "chrome-inconclusive"].map((status) => [status, checked.filter((row) => row.chrome.status === status).length])) : null;
    const differs = checked.filter((row) => row.chrome.status === "differs").map(({ id, outcome, chrome: c }) => ({ id, fast: outcome, chrome: c.outcome }));
    const cellsOut = {};
    for (const row of rows) {
      const cell = (cellsOut[`${row.code}|${row.context}`] ??= { code: row.code, context: row.context, programs: 0, ...tally([]), verdicts: {} });
      cell.programs += 1;
      cell[row.outcome] += 1;
      cell.verdicts[String(row.seed)] = row.outcome;
    }
    const report = { mode: "legal", seed: options.seed, perCell: options.grid ? null : options.perCell, sampling: options.grid ? "grid" : "random", ...entryVetoSummary(values), total: tally(rows), confirmed, chrome, chromeDiffers: differs, cells: cellsOut,
      falsePositives: candidates.map(({ id, code, context, variant, binding, layer, detail, chrome }) => ({ id, code, context, variant, binding, layer, rules: detail.rules, chrome })),
      notLegal: rows.filter((row) => row.outcome === "not-legal").map(({ id, detail }) => ({ id, reported: detail.reported })) };
    writeFileSync(join(out, "legal.json"), `${JSON.stringify(report, null, 2)}\n`);
    writeFileSync(join(out, "programs.jsonl"), `${rows.map((row) => JSON.stringify(row)).join("\n")}\n`);
    console.error(`legal: ${JSON.stringify(report.total)}${chrome ? `; Chrome ${JSON.stringify(chrome)}` : ""}`);
    for (const d of differs) console.error(`legal: Chrome differs ${d.id}: fast ${d.fast}, Chrome ${d.chrome}`);
    for (const fp of report.falsePositives) console.error(`legal: candidate false positive ${fp.id} [${fp.rules.join(", ")}]${fp.chrome ? ` chrome ${fp.chrome.status}` : ""}`);
    for (const row of report.notLegal.slice(0, 10)) console.error(`legal: not legal ${row.id} (Solid reports ${row.reported.join(", ")})`);
    if (confirmed) { console.error("\nSTOP: Chrome confirms a violation where the call is legal (a false positive). Report the program(s) above."); return 3; }
    return 0;
  } finally {
    await config.fast?.close();
  }
}

const usage = `usage: node scripts/fuzz/legal.mjs --out <fresh dir> [--seed N] [--per-cell N | --grid [--layers a,b] [--mounts root,child,show,for] [--sources local,props,context,module,async] [--combine]] [--codes A,B] [--confirm-with-chrome [--chrome-sample 2] [--chrome-strata]]
  [--chrome-concurrency N]   Chrome pages at a time (default the oracle's: at most 8)
  [--runtime-diff <N|all> [--runtime-diff-axes a,b]]   compare Happy DOM's runtime record with Chrome's on up to N programs per stratum, or all
  [--checker-bin <solid-checker>] [--typefacts <solid-typefacts>] [--tsc <tsrs>] [--cache-dir <dir>] [--no-cache]`;

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const options = parseArgs(process.argv.slice(2), { extraValues: ["ids", "per-cell", "codes", "cache-dir", "chrome-sample", "layers", "mounts", "sources", "runtime-diff", "runtime-diff-axes", "chrome-concurrency"], extraFlags: ["grid", "no-chrome-cache", "combine", "chrome-strata"] });
    for (const key of ["perCell", "chromeSample", "chromeConcurrency"]) if (options[key] !== undefined) options[key] = Number(options[key]);
    options.perCell ??= 10;
    if (options.help || !options.out) { console.error(usage); process.exitCode = options.help ? 0 : 2; }
    else process.exitCode = await runLegal(options);
  } catch (error) {
    console.error(`legal: ${error.message}`);
    process.exitCode = 2;
  }
}
