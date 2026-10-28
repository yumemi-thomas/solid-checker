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
import { withRuntimeCache } from "./cache.mjs";
import { runChrome } from "./chrome.mjs";
import { assertFindingsMapped } from "./checker.mjs";
import { disturbances, runtimeEvents } from "./classify.mjs";
import { codesForRule, ignoredRules, ruleHasRuntimeCounterpart } from "./codes.mjs";
import { buildCase } from "./emit.mjs";
import { generateProgram } from "./generate.mjs";
import { hitCount } from "./hits.mjs";
import { fnv, regionOf, siteOf } from "./matrix.mjs";
import { LAYERS, LEGAL_CODES, SPEC, gridBindingsOf, legalVariantsOf } from "./misuse.mjs";
import { evaluate } from "./pipeline.mjs";
import { programSeed } from "./rng.mjs";
import { attachDaemon, configFrom, parseArgs } from "./run.mjs";
import { requireTypeChecker, resolveTsc, typeErrorRefusal } from "./typecheck.mjs";

export const LEGAL_OUTCOMES = ["typed-away", "harness", "not-legal", "disturbed", "false-positive", "unexecuted", "uncertifiable", "clean"];

const inLines = (finding, lines) => lines.some((line) => finding.line <= line && line <= finding.endLine);

/**
 * Judges one legal program at its site.
 * @param {{ code: string, site: object, typecheck: object, checker: object, runtime: object, plan?: object }} input
 */
export function judgeLegal({ code, site, typecheck: typed, checker, runtime, plan }) {
  if (typed && typed.ok === false) return { outcome: "typed-away", detail: { error: typed.errors?.[0] } };
  if (!checker?.ok) return { outcome: "harness", detail: { why: `checker: ${String(checker?.error ?? "not run").slice(0, 120)}` } };
  if (!runtime) return { outcome: "harness", detail: { why: "no runtime record" } };
  const events = runtimeEvents(runtime);
  const region = regionOf(site);
  const relevant = checker.findings.filter((finding) => !ignoredRules.includes(finding.rule));
  const violations = relevant.filter((finding) => finding.kind === "violation");
  const atSite = violations.filter((finding) => inLines(finding, region));
  const stray = violations.filter((finding) => !inLines(finding, region)).map((finding) => `${finding.rule}@${finding.line}`);
  const detail = { findings: relevant.filter((finding) => inLines(finding, region)).map((finding) => `${finding.rule}/${finding.kind}@${finding.line}`), ...(stray.length ? { stray } : {}) };
  if (events.some((event) => event.code === code)) return { outcome: "not-legal", detail: { ...detail, reported: [...new Set(events.map((event) => event.code))] } };
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
  if (ran === 0) return { outcome: "unexecuted", detail };
  return { outcome: "false-positive", detail: { ...detail, rules: [...new Set(contradicted.map((finding) => finding.rule))] } };
}

/** The cells: every code with legal contexts, each legal context. */
export function legalCells({ codes } = {}) {
  return LEGAL_CODES().filter((code) => !codes || codes.includes(code)).flatMap((code) => SPEC[code].legal.map((context) => ({ code, context })));
}

/** The legal shapes of a code (`--grid`): its legal variants x its grid bindings x the layers. */
export const legalShapes = (code) => legalVariantsOf(code).flatMap((variant) => gridBindingsOf(code, variant).flatMap((binding) => LAYERS.map((layer) => ({ variant, binding, layer }))));

/** One legal program: its seed depends on the run seed and the cell and the index (random) or the shape (grid) only. */
export function generateLegal(options, { code, context }, { index, shape }) {
  const key = shape ? `legal/${code}/${context}/${shape.variant ?? "-"}/${shape.binding}/${shape.layer}` : `legal/${code}/${context}`;
  const seed = programSeed((Number(options.seed) ^ fnv(key)) >>> 0, shape ? 0 : index);
  const program = generateProgram(seed, { profile: "misuse", misusePlan: { code, context, legal: true, ...(shape ?? {}) } });
  const suffix = shape ? `${shape.variant ?? "x"}-${shape.binding}-${shape.layer.replace("+", "-")}` : String(index);
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
    const entries = [];
    for (const cell of cells) {
      if (options.grid) legalShapes(cell.code).forEach((shape, index) => entries.push({ cell, index, ...generateLegal(options, cell, { shape }) }));
      else for (let index = 0; index < options.perCell; index += 1) entries.push({ cell, index, ...generateLegal(options, cell, { index }) });
    }
    console.error(`legal: ${cells.length} cells, ${entries.length} programs`);
    const rows = [];
    const values = new Map();
    for (let at = 0; at < entries.length; at += 150) {
      const chunk = entries.slice(at, at + 150);
      const results = await evaluate(chunk.map(({ program, built }) => ({ program, built })), config);
      for (const { cell, program, built } of chunk) {
        const value = results.get(built.id);
        const site = siteOf(built);
        const verdict = value?.error ? { outcome: "harness", detail: { why: value.error } } : judgeLegal({ code: cell.code, site, typecheck: value.typecheck, checker: value.checker, runtime: value.runtime, plan: built.plan });
        const misuse = program.components[0].misuse;
        rows.push({ id: built.id, seed: program.seed, code: cell.code, context: cell.context, variant: misuse.variant, binding: misuse.binding, layer: misuse.layer, outcome: verdict.outcome, detail: verdict.detail, site: { line: site.line, hit: site.hit, aux: site.aux } });
        values.set(built.id, value);
      }
      console.error(`legal: ${Math.min(at + 150, entries.length)}/${entries.length}, ${JSON.stringify(tally(rows))}`);
    }
    assertFindingsMapped();
    const refusal = typeErrorRefusal(rows.filter((row) => row.outcome === "typed-away").length, rows.length);
    if (refusal) throw new Error(refusal);
    // Chrome: every false-positive candidate, its twin run with the hit ledger beside its clean control.
    let confirmed = 0;
    const candidates = rows.filter((row) => row.outcome === "false-positive");
    if (options.confirmWithChrome && candidates.length) {
      const cases = candidates.map((row) => ({ id: row.id, dir: values.get(row.id).caseDir, sites: values.get(row.id).built.sites }));
      const records = await runChrome(cases, { out, install: config.install, bin: config.bin, typefacts: config.typefacts, browser: options.browser, concurrency: options.concurrency, withClean: true });
      for (const row of candidates) {
        const value = values.get(row.id);
        const record = records.get(row.id);
        const judged = record?.checker?.ok && !record.checker.perturbed?.length ? judgeLegal({ code: row.code, site: siteOf(value.built), typecheck: { ok: true }, checker: record.checker, runtime: record.runtime, plan: value.built.plan }) : null;
        row.chrome = judged ? { status: judged.outcome === "false-positive" ? "confirmed" : "not-confirmed", outcome: judged.outcome } : { status: "chrome-inconclusive", why: record?.checker?.perturbed?.[0] ?? record?.runtime?.failure?.message ?? "no record" };
        if (row.chrome.status === "confirmed") confirmed += 1;
      }
    }
    const cellsOut = {};
    for (const row of rows) {
      const cell = (cellsOut[`${row.code}|${row.context}`] ??= { code: row.code, context: row.context, programs: 0, ...tally([]), verdicts: {} });
      cell.programs += 1;
      cell[row.outcome] += 1;
      cell.verdicts[String(row.seed)] = row.outcome;
    }
    const report = { mode: "legal", seed: options.seed, perCell: options.grid ? null : options.perCell, sampling: options.grid ? "grid" : "random", total: tally(rows), confirmed, cells: cellsOut,
      falsePositives: candidates.map(({ id, code, context, variant, binding, layer, detail, chrome }) => ({ id, code, context, variant, binding, layer, rules: detail.rules, chrome })),
      notLegal: rows.filter((row) => row.outcome === "not-legal").map(({ id, detail }) => ({ id, reported: detail.reported })) };
    writeFileSync(join(out, "legal.json"), `${JSON.stringify(report, null, 2)}\n`);
    writeFileSync(join(out, "programs.jsonl"), `${rows.map((row) => JSON.stringify(row)).join("\n")}\n`);
    console.error(`legal: ${JSON.stringify(report.total)}${options.confirmWithChrome ? `; Chrome confirmed ${confirmed} of ${candidates.length}` : ""}`);
    for (const fp of report.falsePositives) console.error(`legal: candidate false positive ${fp.id} [${fp.rules.join(", ")}]${fp.chrome ? ` chrome ${fp.chrome.status}` : ""}`);
    for (const row of report.notLegal.slice(0, 10)) console.error(`legal: not legal ${row.id} (Solid reports ${row.reported.join(", ")})`);
    if (confirmed) { console.error("\nSTOP: Chrome confirms a violation where the call is legal (a false positive). Report the program(s) above."); return 3; }
    return 0;
  } finally {
    await config.fast?.close();
  }
}

const usage = `usage: node scripts/fuzz/legal.mjs --out <fresh dir> [--seed N] [--per-cell N | --grid] [--codes A,B] [--confirm-with-chrome]
  [--checker-bin <solid-checker>] [--typefacts <solid-typefacts>] [--tsc <tsrs>] [--cache-dir <dir>] [--no-cache]`;

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const options = parseArgs(process.argv.slice(2), { extraValues: ["per-cell", "codes", "cache-dir"], extraFlags: ["grid"] });
    if (options.perCell !== undefined) options.perCell = Number(options.perCell);
    options.perCell ??= 10;
    if (options.help || !options.out) { console.error(usage); process.exitCode = options.help ? 0 : 2; }
    else process.exitCode = await runLegal(options);
  } catch (error) {
    console.error(`legal: ${error.message}`);
    process.exitCode = 2;
  }
}
