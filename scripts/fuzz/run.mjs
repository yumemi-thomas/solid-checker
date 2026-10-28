#!/usr/bin/env node
// The differential fuzzer's command line. See scripts/fuzz/README.md.
//
//   node scripts/fuzz/run.mjs --seed 1 --count 50 --out /tmp/fuzz-run-1 \
//     --checker-bin <solid-checker> --typefacts <solid-typefacts> [--confirm-with-chrome] [--shrink]
//
// Pipeline per program: generate -> type-check (tsrs) -> checker verdicts -> Happy DOM runtime
// records -> classify. Only candidates go to Chrome (scripts/runtime-oracle.mjs, the authority) and
// to the shrinker. A violation at a site the ledger shows never ran is a `candidate-false-positive: unexecuted`
// (counted on every run); Chrome confirms it as `confirmed-unexecuted`. Exit status: 0 nothing to stop for, 3 a confirmed false
// positive or a confirmed-unexecuted violation (stop and report, see the README), 4 a confirmed false clean with a coarse key the ledger does not know (report; the sweep
// is complete), 2 a usage or harness error.
import { existsSync, mkdirSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { DEFAULT_ORACLE_INSTALL, chromeStatus, runChrome } from "./chrome.mjs";
import { dedupe } from "./dedupe.mjs";
import { buildCase, writeCase } from "./emit.mjs";
import { DEFAULT_FAST_INSTALL } from "./fast.mjs";
import { GENERATOR_VERSION, generateProgram } from "./generate.mjs";
import { findingsDiff, hitSummary } from "./hits.mjs";
import { classifyChrome, evaluate, hasPackages, judgeEntries, judgeUnexecutedInChrome, signaturesOf } from "./pipeline.mjs";
import { summarizeUnexecuted } from "./unexecuted.mjs";
import { applyLedgerToResults, falseCleanStream } from "./falseclean-run.mjs";
import { LEDGER_PATH, loadLedger } from "./falseclean.mjs";
import { programSeed } from "./rng.mjs";
import { shrink } from "./shrink.mjs";
import { requireTypeChecker, resolveTsc, typeErrorRefusal } from "./typecheck.mjs";
import { isSite, walk } from "./model.mjs";
import { availableParallelism } from "node:os";
import { createFastDaemon } from "./fast-daemon.mjs";
import { mapPoolSettled } from "../lib/pool.mjs";
import { cleanupEvidence } from "./disposal.mjs";
import { containerEvidence } from "./container.mjs";

const FLAGS = new Set(["confirm-with-chrome", "shrink", "keep-work", "generate-only", "control", "dead-site", "no-instrument", "no-daemon", "no-cache", "stop-on-mismatch", "no-ledger", "all-cores", "containers", "real-clock", "help"]);
const VALUES = new Set(["seed", "count", "out", "profile", "install", "fast-install", "checker-bin", "typefacts", "tsc", "workers", "concurrency", "browser", "budget", "shrink-mode", "components", "shrink-jobs", "shrink-batch", "shrink-control", "ledger", "code", "context", "binding", "layer", "variant"]);

export function parseArgs(argv, { extraValues = [], extraFlags = [] } = {}) {
  const options = { seed: 1, count: 20, shrinkMode: "discovery", budget: 120 };
  for (let index = 0; index < argv.length; index += 1) {
    const raw = argv[index];
    if (!raw.startsWith("--")) throw new Error(`unexpected argument ${raw}`);
    const key = raw.slice(2);
    const camel = key.replace(/-([a-z])/g, (_, c) => c.toUpperCase());
    if (FLAGS.has(key) || extraFlags.includes(key)) { options[camel] = true; continue; }
    if (!VALUES.has(key) && !extraValues.includes(key)) throw new Error(`unknown option ${raw}`);
    if (argv[index + 1] === undefined) throw new Error(`${raw} needs a value`);
    index += 1;
    options[camel] = argv[index];
  }
  for (const key of ["seed", "count", "workers", "concurrency", "budget", "components", "shrinkJobs", "shrinkBatch"]) if (options[key] !== undefined) options[key] = Number(options[key]);
  for (const key of ["seed", "count", "budget"]) if (!Number.isInteger(options[key]) || options[key] < 0) throw new Error(`--${key} needs a non-negative integer`);
  if (!["discovery", "focused"].includes(options.shrinkMode)) throw new Error("--shrink-mode is discovery or focused");
  if (options.shrinkControl !== undefined && !["accepted", "each"].includes(options.shrinkControl)) throw new Error("--shrink-control is accepted or each");
  return options;
}

/** Half the cores for the checker and type-check pool: every unit starts a checker and a type checker (more than this gained nothing in the sweep). */
export const defaultParallelism = (cores = availableParallelism(), { allCores = false } = {}) => (allCores ? Math.max(1, cores) : Math.max(1, Math.floor(cores / 2)));

/**
 * Vitest workers. A program waits on timers (about 0.7 s of sleeping against some 0.1 s of work), so workers do not compete for cores:
 * the knee measured on 14 cores with 100 programs was at about twice the cores (8 workers 15 s, 16 workers 8.5 s, 32 workers 6.8 s, 48 workers 6.0 s,
 * checker concurrency unchanged). On the virtual clock a program does not wait, so a worker is a core.
 */
export const defaultWorkers = (cores = availableParallelism(), { virtualClock = false } = {}) => Math.max(1, virtualClock ? cores : cores * 2);

/** Starts the long-lived fast host unless `--no-daemon`; the caller closes it (`config.fast?.close()`). */
export function attachDaemon(config, options) {
  return options.noDaemon ? config : { ...config, fast: createFastDaemon({ install: config.fastInstall, workers: config.workers, virtualClock: config.virtualClock }) };
}

/** The run's configuration from parsed options (`out` is the fresh output directory). */
export function configFrom(options, out) {
  return {
    install: resolve(options.install ?? DEFAULT_ORACLE_INSTALL),
    fastInstall: resolve(options.fastInstall ?? DEFAULT_FAST_INSTALL),
    bin: options.checkerBin ? resolve(options.checkerBin) : process.env.SOLID_CHECKER_NATIVE_BIN,
    typefacts: options.typefacts ? resolve(options.typefacts) : process.env.SOLID_TYPEFACTS_BIN,
    tsc: resolveTsc(options.tsc),
    casesDir: join(out, "cases"),
    concurrency: options.concurrency ?? defaultParallelism(undefined, { allCores: options.allCores }),
    workers: options.workers ?? defaultWorkers(undefined, { virtualClock: !options.realClock }),
    keepWork: Boolean(options.keepWork),
    // The checker's neutrality control on the twin is how a case with a generated package is vetted for Chrome (it cannot ask the oracle).
    control: Boolean(options.control) || options.profile === "packages",
    deadSite: Boolean(options.deadSite),
    shrinkControl: options.shrinkControl ?? "accepted",
    virtualClock: !options.realClock,
    instrument: !options.noInstrument,
  };
}

/** The programs of a seed range, built as cases. */
export function generateEntries(options) {
  const entries = [];
  for (let index = 0; index < options.count; index += 1) {
    const seed = programSeed(options.seed, index);
    const misusePlan = options.profile === "misuse" ? Object.fromEntries(["code", "context", "binding", "layer", "variant"].filter((key) => options[key] !== undefined).map((key) => [key, options[key]])) : undefined;
    const program = generateProgram(seed, { profile: options.profile, components: options.components, containers: options.containers, misusePlan });
    entries.push({ program, built: buildCase(program, { id: `fuzz-${seed}` }) });
  }
  return entries;
}

const containerRows = (value) => {
  const rows = containerEvidence(value.program, value.built.ledger, value.runtime?.hits ?? null);
  return rows.length ? rows : undefined;
};

/** One results.jsonl row per evaluated program. */
export function resultRows(results) {
  const rows = [];
  for (const [id, value] of results) {
    rows.push({
      id, seed: value.program.seed, verdict: value.classification.verdict, reasons: value.classification.reasons,
      items: value.classification.items, sites: value.built.sites.length, typeErrors: value.typecheck?.ok === false ? value.typecheck.errors : undefined,
      checkerMs: value.checker?.wallMs, runtimeMs: value.runtime?.wallMs,
      hits: value.runtime ? hitSummary(value.built.ledger, value.runtime.hits ?? null, value.runtime.witnesses, value.runtime.quiescent ?? null) : undefined,
      cleanup: value.runtime ? cleanupEvidence(value.program, value.built.ledger, value.runtime.hits) ?? undefined : undefined,
      shape: value.program.components[0]?.shape,
      containers: containerRows(value),
      control: value.control ? { matched: value.control.matched, diffs: value.control.diffs } : undefined,
    });
  }
  return rows;
}

const usage = `usage: node scripts/fuzz/run.mjs --out <fresh dir> [--seed N] [--count N] [--profile name] [--components N]
  [--checker-bin <solid-checker>] [--typefacts <solid-typefacts>]   (or SOLID_CHECKER_NATIVE_BIN, SOLID_TYPEFACTS_BIN)
  [--install <oracle install>] [--fast-install <fuzz install>] [--tsc <tsrs>] [--workers N] [--concurrency N]
  [--confirm-with-chrome [--browser <chromium>]] [--shrink [--shrink-mode discovery|focused] [--budget N]]
  [--ledger <known-open.json>] [--no-ledger]   the accepted false cleans (default scripts/fuzz/known-open.json)
  [--control]         neutrality control: also run the clean twin and the checker on the instrumented text, and compare
  [--dead-site]       with --confirm-with-chrome: confirm an unexecuted candidate only under the strict closed grammar (no containers, no dead-code waiver)
  [--containers]      wrap statements in containers (container.mjs) on top of any profile; --profile containers draws the base profile too
  [--all-cores]       every core for the checker/type-check concurrency (the default is half the cores); the Vitest workers default to twice the cores (--workers N)
  [--shrink-jobs N]   shrink N signature groups at once (default 4); [--shrink-batch N] candidates judged per round (default 8)
  [--shrink-control accepted|each]   with --control, ask for the control of each accepted reduction (default) or of every candidate tried
  [--real-clock]      the fast path waits on the wall clock (the default is the virtual clock, fast/virtual-clock.mjs); Chrome always uses its own
  [--no-daemon]       one Vitest process per batch instead of the long-lived fast host
  [--no-instrument]   run the clean text with no hit ledger (the baseline of the neutrality regression)
  [--generate-only]   write and type-check the programs, run nothing else
  [--keep-work]       keep the materialized checker directories`;

async function main(options) {
  if (options.help) { console.log(usage); return 0; }
  if (!options.out) throw new Error(`--out <fresh directory> is required\n${usage}`);
  const out = resolve(options.out);
  if (existsSync(out)) throw new Error(`use a fresh --out path (${out} exists)`);
  requireTypeChecker(resolveTsc(options.tsc));
  mkdirSync(out, { recursive: true });

  const config = attachDaemon(configFrom(options, out), options);
  try {
    return await sweep(options, config, out);
  } finally {
    await config.fast?.close();
  }
}

async function sweep(options, config, out) {
  if (!existsSync(join(config.install, "node_modules/solid-js/package.json"))) {
    throw new Error(`${config.install} has no node_modules/solid-js: run \`make runtime-oracle-install\``);
  }

  if (options.noInstrument && options.confirmWithChrome) throw new Error("--no-instrument runs the clean text; Chrome confirmation reads the hit ledger, so the two do not combine");
  const entries = generateEntries(options);
  if (options.generateOnly) {
    for (const { built } of entries) writeCase(join(config.casesDir, built.id), built);
  }

  // A bad ledger stops the sweep before it starts: it must not quiet anything by accident.
  const ledger = options.generateOnly || options.noLedger ? [] : loadLedger(options.ledger ? resolve(options.ledger) : LEDGER_PATH);
  const results = options.generateOnly ? await typecheckOnly(entries, config) : await evaluate(entries, config);
  const firedEntries = options.generateOnly ? [] : applyLedgerToResults(results, ledger);
  const rows = resultRows(results);
  writeFileSync(join(out, "results.jsonl"), rows.map((row) => JSON.stringify(row)).join("\n") + "\n");
  const tally = (key) => rows.reduce((counts, row) => ({ ...counts, [row[key]]: (counts[row[key]] ?? 0) + 1 }), {});
  const itemTally = rows.flatMap((row) => row.items).reduce((counts, item) => ({ ...counts, [item.kind]: (counts[item.kind] ?? 0) + 1 }), {});
  console.error(`programs ${rows.length}: ${JSON.stringify(tally("verdict"))}; items ${JSON.stringify(itemTally)}`);
  const refusal = typeErrorRefusal(rows.filter((row) => row.verdict === "type-error" || row.typeErrors).length, rows.length, { first: rows.find((row) => row.typeErrors)?.typeErrors[0] ?? rows.find((row) => row.verdict === "type-error")?.reasons?.[0] });
  if (refusal) throw new Error(refusal);

  const candidates = [...results.values()].filter((value) => value.classification.verdict === "candidate-false-positive");
  const findings = [];
  let confirmed = 0;
  let confirmedUnexecuted = 0;
  const unexecuted = [];
  // Reductions asked for, one per (kind, signature): the first program of each signature is its representative.
  const wanted = new Map();
  const wantShrink = (value, spec) => {
    const key = `${spec.kind}|${spec.signature}`;
    if (!wanted.has(key)) wanted.set(key, { value, ...spec });
    return key;
  };
  let falseClean = null;
  if (!options.generateOnly) {
    // Chrome decides. Without it a candidate stays a candidate.
    const chromeOptions = { out, install: config.install, bin: config.bin, typefacts: config.typefacts, browser: options.browser, concurrency: options.concurrency };
    const chromeRecords = async (values) => {
      // A case with a generated package is judged by the checker as a user runs it (the oracle's feedback run does not read packages
      // from source): its findings on the clean text, and on the twin as the neutrality control.
      const chromeCases = values.map((value) => ({
        id: value.built.id, dir: value.caseDir ?? join(config.casesDir, value.built.id), sites: value.built.sites,
        ...(hasPackages(value.built) ? { checker: { ok: true, findings: value.checker.findings, perturbed: value.twinChecker?.ok ? findingsDiff(value.checker.findings, value.twinChecker.findings) : ["no checker run on the twin (use --control)"] } } : {}),
      }));
      // Each case runs twice in Chrome in one oracle call: the twin, and the clean case, whose findings are what the twin's must equal
      // (the oracle reports kinds the fast checker adapter does not, so the fast pipeline's findings are not the reference).
      return runChrome(chromeCases, { ...chromeOptions, withClean: true });
    };
    // Unexecuted candidates (a violation at a site the ledger shows never ran) are a hypothesis on every run; Chrome confirms them.
    const unexecutedValues = [...results.values()].filter((value) => value.classification.items.some((item) => item.kind === "candidate-unexecuted"));
    const chromeSet = [...new Set([...candidates, ...(options.confirmWithChrome ? unexecutedValues : [])])];
    const chrome = options.confirmWithChrome && chromeSet.length ? await chromeRecords(chromeSet) : new Map();
    // The clean-twin control Chrome's records cannot supply: run for the unexecuted candidates that were not controlled already.
    const uncontrolled = options.confirmWithChrome ? unexecutedValues.filter((value) => !value.control) : [];
    const controlled = uncontrolled.length
      ? await evaluate(uncontrolled.map(({ program, built }) => ({ program, built })), { ...config, control: true, casesDir: join(out, "control-cases") })
      : new Map();
    for (const value of candidates) {
      const record = chrome.get(value.built.id);
      const judged = record ? classifyChrome(value, record, config) : null;
      const status = chromeStatus(judged);
      if (status === "confirmed") confirmed += 1;
      const reducible = options.shrink && !["refuted-by-chrome", "chrome-inconclusive"].includes(status);
      for (const found of signaturesOf(value.program, value.built, judged ?? value.classification)) {
        findings.push({ id: value.built.id, signature: found.signature, status, size: value.built.text.split("\n").length, rule: found.item.finding?.rule, reason: found.item.reason, repro: join(config.casesDir, value.built.id),
          ...(reducible ? { shrinkKey: wantShrink(value, { kind: "candidate-false-positive", signature: found.signature }) } : {}) });
      }
      if (judged && ["refuted-by-chrome", "chrome-inconclusive"].includes(status)) findings.push({ id: value.built.id, signature: "-", status, reason: judged.reasons.join("; ") });
    }
    // The unexecuted candidates: Chrome's answer per item, one row each (a program can hold several).
    for (const value of unexecutedValues) {
      const record = chrome.get(value.built.id);
      const matched = (value.control ?? controlled.get(value.built.id)?.control)?.matched ?? null;
      for (const item of value.classification.items.filter((row) => row.kind === "candidate-unexecuted")) {
        const judgement = !options.confirmWithChrome ? { status: "unconfirmed", why: "no-chrome" }
          : record ? judgeUnexecutedInChrome(value, item, record, { controlMatched: matched, strict: Boolean(options.deadSite) }) : { status: "chrome-inconclusive", why: "no-record" };
        if (judgement.status === "confirmed-unexecuted") confirmedUnexecuted += 1;
        const reducible = options.shrink && !["refuted-by-chrome", "chrome-inconclusive"].includes(judgement.status);
        const found = signaturesOf(value.program, value.built, { items: [item] })[0];
        unexecuted.push({ id: value.built.id, signature: found?.signature ?? "-", rule: item.finding.rule, finding: item.finding.id, line: item.finding.line, bases: item.bases, ...judgement,
          size: value.built.text.split("\n").length, repro: join(config.casesDir, value.built.id),
          ...(reducible && found ? { shrinkKey: wantShrink(value, { kind: "candidate-unexecuted", signature: found.signature }) } : {}) });
      }
    }
    // A real defect named by the wrong rule is reported, not confirmed or shrunk.
    for (const value of results.values()) {
      for (const found of signaturesOf(value.program, value.built, { items: value.classification.items.filter((item) => item.kind === "candidate-wrong-rule") })) {
        findings.push({ id: value.built.id, signature: found.signature, status: "wrong-rule-provisional", reason: found.item.reason, size: value.built.text.split("\n").length, repro: join(config.casesDir, value.built.id) });
      }
    }
    // The reductions: one representative per signature (the later members of a group add a count, not a reduction), run in a
    // bounded pool across groups. Each reduction batches its candidates (shrinkCandidate).
    const keys = [...wanted.keys()];
    const reduced = await mapPoolSettled(keys, async (key) => shrinkCandidate(wanted.get(key).value, config, options, out, wanted.get(key)), { concurrency: options.shrinkJobs ?? Math.min(4, keys.length || 1) });
    const minimals = new Map(keys.map((key, index) => [key, reduced[index].value ?? null]));
    for (const [key, outcome] of keys.map((key, index) => [key, reduced[index]])) if (outcome.error) console.error(`shrink ${key} failed: ${outcome.error.message ?? outcome.error}`);
    for (const finding of [...findings, ...unexecuted]) {
      if (!finding.shrinkKey) continue;
      const minimal = minimals.get(finding.shrinkKey);
      if (minimal) { finding.size = minimal.size; finding.repro = minimal.dir; }
      delete finding.shrinkKey;
    }
    // False cleans: their own stream (falseclean-run.mjs), below.
    falseClean = await falseCleanStream({ results, ledger, firedEntries, config, options, out, chromeRecords });
  }
  const groups = dedupe(findings.filter((f) => f.signature !== "-").map((f) => ({ ...f })));
  const unexecutedReport = options.generateOnly ? null : summarizeUnexecuted(unexecuted, dedupe);
  const falseCleanReport = falseClean && {
    groups: falseClean.groups.map(({ coarseKey, code, severity, count, status, signature, repro, size, minimum, members }) => ({ coarseKey, code, severity, count, status, signature, repro, size, minimum, members })),
    knownOpen: falseClean.knownOpen, staleOpen: falseClean.staleOpen.map(({ code, coarseKey, ref }) => ({ code, coarseKey, ref })), recallGaps: falseClean.recallGaps,
  };
  writeFileSync(join(out, "findings.json"), `${JSON.stringify({ generator: GENERATOR_VERSION, seed: options.seed, count: options.count, groups, refuted: findings.filter((f) => f.signature === "-"), falseClean: falseCleanReport, unexecuted: unexecutedReport }, null, 2)}\n`);
  writeFileSync(join(out, "summary.json"), `${JSON.stringify({ programs: rows.length, verdicts: tally("verdict"), items: itemTally, candidates: candidates.length, confirmed, groups: groups.length,
    unexecuted: unexecutedReport && { candidates: unexecutedReport.candidates, programs: unexecutedReport.programs, confirmed: unexecutedReport.confirmed, byStatus: unexecutedReport.byStatus, byBasis: unexecutedReport.byBasis },
    falseClean: falseClean && { groups: falseClean.groups.length, confirmed: falseClean.confirmed, knownOpen: falseClean.knownOpen, staleOpen: falseClean.staleOpen.length, recallGaps: falseClean.recallGaps.reduce((sum, row) => sum + row.count, 0) } }, null, 2)}\n`);
  for (const group of groups) console.error(`${group.count}x [${group.representative.status}] ${group.signature}\n    repro: ${group.representative.repro}`);
  if (falseClean) reportFalseCleans(falseClean);
  if (unexecutedReport?.candidates) reportUnexecuted(unexecutedReport);

  if (confirmed) {
    console.error("\nSTOP: Chrome confirms a checker violation the real runtime contradicts. Do not continue the sweep; report the repro(s) above (scripts/fuzz/README.md).");
    return 3;
  }
  if (confirmedUnexecuted) {
    console.error("\nSTOP: Chrome confirms a checker violation at a site no execution enters (confirmed-unexecuted: ADR 0458 says it is uncertifiable). Do not continue the sweep; report the repro(s) above (scripts/fuzz/README.md).");
    return 3;
  }
  if (falseClean?.confirmed) {
    console.error(`\nFALSE CLEAN: Chrome confirms ${falseClean.confirmed} defect(s) Solid reports and the checker misses, with a coarse key the ledger does not know (scripts/fuzz/README.md). The sweep ran to the end.`);
    return 4;
  }
  return 0;
}

/** The unexecuted candidates: the count by status and basis, then one line per construct group. */
function reportUnexecuted(report) {
  console.error(`unexecuted (candidate-false-positive: unexecuted): ${report.candidates} item(s) in ${report.programs} program(s); ${JSON.stringify(report.byStatus)}; basis ${JSON.stringify(report.byBasis)}`);
  for (const group of report.groups) console.error(`${group.count}x [${group.representative.status}${group.representative.why ? `: ${group.representative.why}` : ""}] ${group.signature}\n    repro: ${group.representative.repro}`);
}

/** One line per coarse group (worst severity first), then the ledger and recall-gap counts. */
function reportFalseCleans(falseClean) {
  for (const group of falseClean.groups) {
    const minimum = group.minimum ? `, minimum ${group.minimum.status}` : "";
    console.error(`false clean ${group.count}x [${group.status}${minimum}] ${group.severity} ${group.coarseKey}\n    repro: ${group.repro}`);
  }
  console.error(`false cleans: ${falseClean.groups.length} new group(s), ${falseClean.knownOpen} item(s) known-open, ${falseClean.staleOpen.length} ledger entr${falseClean.staleOpen.length === 1 ? "y" : "ies"} not fired in this run (stale-open if the run covered their profile)`);
  const gaps = falseClean.recallGaps.reduce((sum, row) => sum + row.count, 0);
  if (gaps) console.error(`recall gaps (the checker declines where Solid reports): ${gaps}; top ${falseClean.recallGaps.slice(0, 3).map((row) => `${row.count}x ${row.signature}`).join("; ")}`);
}

async function typecheckOnly(entries, config) {
  const { mapPoolSettled } = await import("../lib/pool.mjs");
  const { materializeForChecker } = await import("./checker.mjs");
  const { typecheck } = await import("./typecheck.mjs");
  const { rmSync } = await import("node:fs");
  const results = new Map();
  const settled = await mapPoolSettled(entries, async ({ built }) => {
    const dir = materializeForChecker(built, config.install, built.id);
    const types = await typecheck(dir, { tsc: config.tsc });
    // The instrumented twin is what runs: it must type-check whenever the clean text does.
    if (types.ok && config.instrument !== false) {
      const twin = materializeForChecker(built, config.install, `${built.id}-run`, { twin: true });
      const twinTypes = await typecheck(twin, { tsc: config.tsc });
      if (!config.keepWork) rmSync(twin, { recursive: true, force: true });
      if (!twinTypes.ok) types.errors = [`the instrumented twin does not type-check: ${twinTypes.errors[0]}`], types.ok = false;
    }
    if (!config.keepWork) rmSync(dir, { recursive: true, force: true });
    return types;
  }, { concurrency: config.concurrency });
  entries.forEach(({ program, built }, index) => {
    const types = settled[index].value ?? { ok: false, errors: [String(settled[index].error)] };
    results.set(built.id, { program, built, typecheck: types, classification: { verdict: types.ok ? "generated" : "type-error", reasons: types.errors ?? [], items: [] } });
  });
  return results;
}

/** Whether every site's witness selector (`[data-probe="w1"]`) is still an attribute in the text: the scenario waits for it in Chrome. */
export const witnessesPresent = (built) => built.sites.every((site) => !site.witness || built.text.includes(String(site.witness.selector).slice(1, -1).replace(/\\"/g, '"')));

/**
 * Shrinks one candidate under the fast path (checker + Happy DOM); writes the minimal case.
 * Each round of candidates is judged in one `evaluate` (one fast-path run for the lot, the checker and type check in a pool).
 */
export async function shrinkCandidate(value, config, options, out, { kind = "candidate-false-positive" } = {}) {
  const target = value.classification.items.find((item) => item.kind === kind);
  const targetSignature = signaturesOf(value.program, value.built, { items: [target] })[0]?.signature;
  let counter = 0;
  const base = `${value.built.id}-fp`;
  const holds = (program, built, result) => {
    const hit = result.classification.items.filter((item) => item.kind === kind);
    if (!hit.length) return false;
    // An unexecuted site proves nothing about its witness, so a reduction could drop the probe the scenario waits for and leave a repro
    // Chrome cannot run: every witness selector must still be in the text.
    if (kind === "candidate-unexecuted" && !witnessesPresent(built)) return false;
    if (options.shrinkMode === "discovery") return hit.some((item) => item.finding.rule === target.finding.rule);
    return signaturesOf(program, built, { items: hit }).some((found) => found.signature === targetSignature);
  };
  const judgeBatch = async (programs) => {
    const entries = programs.map((program) => { counter += 1; return { program, built: buildCase(program, { id: `${base}-${counter}` }) }; });
    return judgeEntries(entries, { ...config, casesDir: join(out, "shrink") }, ({ program, built }, result) => holds(program, built, result));
  };
  const cheap = (program) => [...walk(program)].some(({ node }) => isSite(node));
  const reduced = await shrink(value.program, async (program) => (await judgeBatch([program]))[0], { budget: options.budget, cheap, judgeBatch, batchSize: options.shrinkBatch ?? 8 });
  const finalBuilt = buildCase(reduced.program, { id: `${value.built.id}-min`, note: `shrunk from fuzz seed ${value.program.seed} (${reduced.accepted} reductions, ${reduced.attempts} runs)` });
  const dir = writeCase(join(out, "repros", createHash("sha1").update(finalBuilt.text).digest("hex").slice(0, 10)), finalBuilt);
  console.error(`shrunk ${value.built.id}: ${reduced.attempts} runs in ${reduced.rounds} batches, ${reduced.accepted} accepted, ${finalBuilt.text.split("\n").length} lines -> ${dir}`);
  return { dir, size: finalBuilt.text.split("\n").length };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    process.exitCode = await main(parseArgs(process.argv.slice(2)));
  } catch (error) {
    console.error(`fuzz: ${error.message}`);
    process.exitCode = 2;
  }
}
