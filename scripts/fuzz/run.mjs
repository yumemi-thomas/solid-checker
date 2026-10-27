#!/usr/bin/env node
// The differential fuzzer's command line. See scripts/fuzz/README.md.
//
//   node scripts/fuzz/run.mjs --seed 1 --count 50 --out /tmp/fuzz-run-1 \
//     --checker-bin <solid-checker> --typefacts <solid-typefacts> [--confirm-with-chrome] [--shrink]
//
// Pipeline per program: generate -> type-check (tsrs) -> checker verdicts -> Happy DOM runtime
// records -> classify. Only candidates go to Chrome (scripts/runtime-oracle.mjs, the authority) and
// to the shrinker. Exit status: 0 no confirmed false positive, 3 a confirmed false positive (stop and
// report, see the README), 2 a usage or harness error.
import { existsSync, mkdirSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { DEFAULT_ORACLE_INSTALL, chromeStatus, runChrome } from "./chrome.mjs";
import { dedupe } from "./dedupe.mjs";
import { buildCase, writeCase } from "./emit.mjs";
import { DEFAULT_FAST_INSTALL } from "./fast.mjs";
import { GENERATOR_VERSION, generateProgram } from "./generate.mjs";
import { hitSummary } from "./hits.mjs";
import { classifyChrome, evaluate, signaturesOf } from "./pipeline.mjs";
import { programSeed } from "./rng.mjs";
import { shrink } from "./shrink.mjs";
import { DEFAULT_TSC } from "./typecheck.mjs";
import { isSite, walk } from "./model.mjs";
import { availableParallelism } from "node:os";
import { createFastDaemon } from "./fast-daemon.mjs";
import { cleanupEvidence } from "./disposal.mjs";

const FLAGS = new Set(["confirm-with-chrome", "shrink", "keep-work", "generate-only", "control", "dead-site", "no-instrument", "no-daemon", "no-cache", "stop-on-mismatch", "hunt-false-cleans", "help"]);
const VALUES = new Set(["seed", "count", "out", "profile", "install", "fast-install", "checker-bin", "typefacts", "tsc", "workers", "concurrency", "browser", "budget", "shrink-mode", "components"]);

export function parseArgs(argv, { extraValues = [] } = {}) {
  const options = { seed: 1, count: 20, shrinkMode: "discovery", budget: 120 };
  for (let index = 0; index < argv.length; index += 1) {
    const raw = argv[index];
    if (!raw.startsWith("--")) throw new Error(`unexpected argument ${raw}`);
    const key = raw.slice(2);
    const camel = key.replace(/-([a-z])/g, (_, c) => c.toUpperCase());
    if (FLAGS.has(key)) { options[camel] = true; continue; }
    if (!VALUES.has(key) && !extraValues.includes(key)) throw new Error(`unknown option ${raw}`);
    if (argv[index + 1] === undefined) throw new Error(`${raw} needs a value`);
    index += 1;
    options[camel] = argv[index];
  }
  for (const key of ["seed", "count", "workers", "concurrency", "budget", "components"]) if (options[key] !== undefined) options[key] = Number(options[key]);
  for (const key of ["seed", "count", "budget"]) if (!Number.isInteger(options[key]) || options[key] < 0) throw new Error(`--${key} needs a non-negative integer`);
  if (!["discovery", "focused"].includes(options.shrinkMode)) throw new Error("--shrink-mode is discovery or focused");
  return options;
}

/** Half the cores: every unit starts a checker and a type checker, and the runtime path sleeps on timers, so more than this only queues. */
export const defaultParallelism = (cores = availableParallelism()) => Math.max(1, Math.floor(cores / 2));

/** Starts the long-lived fast host unless `--no-daemon`; the caller closes it (`config.fast?.close()`). */
export function attachDaemon(config, options) {
  return options.noDaemon ? config : { ...config, fast: createFastDaemon({ install: config.fastInstall, workers: config.workers }) };
}

/** The run's configuration from parsed options (`out` is the fresh output directory). */
export function configFrom(options, out) {
  return {
    install: resolve(options.install ?? DEFAULT_ORACLE_INSTALL),
    fastInstall: resolve(options.fastInstall ?? DEFAULT_FAST_INSTALL),
    bin: options.checkerBin ? resolve(options.checkerBin) : process.env.SOLID_CHECKER_NATIVE_BIN,
    typefacts: options.typefacts ? resolve(options.typefacts) : process.env.SOLID_TYPEFACTS_BIN,
    tsc: options.tsc ?? process.env.FUZZ_TSC ?? DEFAULT_TSC,
    casesDir: join(out, "cases"),
    concurrency: options.concurrency ?? defaultParallelism(),
    workers: options.workers ?? defaultParallelism(),
    keepWork: Boolean(options.keepWork),
    control: Boolean(options.control),
    deadSite: Boolean(options.deadSite),
    instrument: !options.noInstrument,
  };
}

/** The programs of a seed range, built as cases. */
export function generateEntries(options) {
  const entries = [];
  for (let index = 0; index < options.count; index += 1) {
    const seed = programSeed(options.seed, index);
    const program = generateProgram(seed, { profile: options.profile, components: options.components });
    entries.push({ program, built: buildCase(program, { id: `fuzz-${seed}` }) });
  }
  return entries;
}

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
      control: value.control ? { matched: value.control.matched, diffs: value.control.diffs } : undefined,
    });
  }
  return rows;
}

const usage = `usage: node scripts/fuzz/run.mjs --out <fresh dir> [--seed N] [--count N] [--profile name] [--components N]
  [--checker-bin <solid-checker>] [--typefacts <solid-typefacts>]   (or SOLID_CHECKER_NATIVE_BIN, SOLID_TYPEFACTS_BIN)
  [--install <oracle install>] [--fast-install <fuzz install>] [--tsc <tsrs>] [--workers N] [--concurrency N]
  [--confirm-with-chrome [--browser <chromium>]] [--shrink [--shrink-mode discovery|focused] [--budget N]]
  [--control]         neutrality control: also run the clean twin and the checker on the instrumented text, and compare
  [--dead-site]       EXPERIMENTAL, off by default: the dead-site false-positive subkind (implies --control)
  [--no-daemon]       one Vitest process per batch instead of the long-lived fast host
  [--no-instrument]   run the clean text with no hit ledger (the baseline of the neutrality regression)
  [--hunt-false-cleans] also confirm false cleans in Chrome (with --confirm-with-chrome) and shrink them (with --shrink)
  [--generate-only]   write and type-check the programs, run nothing else
  [--keep-work]       keep the materialized checker directories`;

async function main(options) {
  if (options.help) { console.log(usage); return 0; }
  if (!options.out) throw new Error(`--out <fresh directory> is required\n${usage}`);
  const out = resolve(options.out);
  if (existsSync(out)) throw new Error(`use a fresh --out path (${out} exists)`);
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

  const results = options.generateOnly ? await typecheckOnly(entries, config) : await evaluate(entries, config);
  const rows = resultRows(results);
  writeFileSync(join(out, "results.jsonl"), rows.map((row) => JSON.stringify(row)).join("\n") + "\n");
  const tally = (key) => rows.reduce((counts, row) => ({ ...counts, [row[key]]: (counts[row[key]] ?? 0) + 1 }), {});
  const itemTally = rows.flatMap((row) => row.items).reduce((counts, item) => ({ ...counts, [item.kind]: (counts[item.kind] ?? 0) + 1 }), {});
  console.error(`programs ${rows.length}: ${JSON.stringify(tally("verdict"))}; items ${JSON.stringify(itemTally)}`);

  const candidates = [...results.values()].filter((value) => value.classification.verdict === "candidate-false-positive");
  const findings = [];
  let confirmed = 0;
  if (!options.generateOnly) {
    // Chrome decides. Without it a candidate stays a candidate.
    let chrome = new Map();
    const huntable = options.huntFalseCleans ? [...results.values()].filter((value) => value.classification.items.some((item) => item.kind === "candidate-false-clean")) : [];
    const toChrome = [...new Set([...candidates, ...huntable])];
    if (options.confirmWithChrome && toChrome.length) {
      const chromeCases = toChrome.map((value) => ({ id: value.built.id, dir: join(config.casesDir, value.built.id), sites: value.built.sites }));
      const chromeOptions = { out, install: config.install, bin: config.bin, typefacts: config.typefacts, browser: options.browser, concurrency: options.concurrency };
      // The clean case first, in Chrome: the oracle's own findings on the clean text are what its findings on the twin must equal
      // (the oracle reports kinds the fast checker adapter does not, so the fast pipeline's findings are not the reference).
      const cleanRuns = await runChrome(chromeCases, { ...chromeOptions, control: true });
      chrome = await runChrome(chromeCases.map((c) => ({ ...c, findings: cleanRuns.get(c.id)?.checker?.ok ? cleanRuns.get(c.id).checker.findings : null })), chromeOptions);
    }
    for (const value of candidates) {
      const record = chrome.get(value.built.id);
      const judged = record ? classifyChrome(value, record, config) : null;
      const status = chromeStatus(judged);
      if (status === "confirmed") confirmed += 1;
      let minimal = null;
      if (options.shrink && !["refuted-by-chrome", "chrome-inconclusive"].includes(status)) minimal = await shrinkCandidate(value, config, options, out);
      for (const found of signaturesOf(value.program, value.built, judged ?? value.classification)) {
        findings.push({ id: value.built.id, signature: found.signature, status, size: minimal ? minimal.size : value.built.text.split("\n").length, rule: found.item.finding?.rule, reason: found.item.reason, repro: minimal?.dir ?? join(config.casesDir, value.built.id) });
      }
      if (judged && ["refuted-by-chrome", "chrome-inconclusive"].includes(status)) findings.push({ id: value.built.id, signature: "-", status, reason: judged.reasons.join("; ") });
    }
    // A real defect named by the wrong rule is reported, not confirmed or shrunk.
    for (const value of results.values()) {
      for (const found of signaturesOf(value.program, value.built, { items: value.classification.items.filter((item) => item.kind === "candidate-wrong-rule") })) {
        findings.push({ id: value.built.id, signature: found.signature, status: "wrong-rule-provisional", reason: found.item.reason, size: value.built.text.split("\n").length, repro: join(config.casesDir, value.built.id) });
      }
    }
    // False cleans: a runtime diagnostic no finding covers. Reported as provisional unless hunted (--hunt-false-cleans): then Chrome
    // decides and the shrinker reduces what Chrome confirms, the way a false positive is.
    const shrunk = new Set();
    for (const value of results.values()) {
      const items = value.classification.items.filter((item) => item.kind === "candidate-false-clean");
      if (!items.length) continue;
      const record = options.huntFalseCleans ? chrome.get(value.built.id) : null;
      const judged = record ? classifyChrome(value, record, config) : null;
      const chromeItems = judged ? judged.items.filter((item) => item.kind === "candidate-false-clean") : null;
      const confirmedCodes = chromeItems ? new Set(chromeItems.map((item) => item.diagnostic.code)) : null;
      for (const item of items) {
        let status = "false-clean-provisional";
        if (options.huntFalseCleans) {
          if (!judged) status = "false-clean-unconfirmed";
          else if (confirmedCodes.has(item.diagnostic.code)) status = "false-clean-confirmed";
          else status = judged.verdict === "inconclusive" ? "false-clean-chrome-inconclusive" : "false-clean-refuted-by-chrome";
        }
        let minimal = null;
        const signatureOfItem = signaturesOf(value.program, value.built, { items: [item] })[0]?.signature;
        // One shrink per signature: the later members of a group add a count, not a reduction.
        if (options.shrink && options.huntFalseCleans && ["false-clean-confirmed", "false-clean-unconfirmed"].includes(status) && !shrunk.has(signatureOfItem)) {
          shrunk.add(signatureOfItem);
          minimal = await shrinkCandidate(value, config, options, out, { kind: "candidate-false-clean", code: item.diagnostic.code });
        }
        for (const found of signaturesOf(value.program, value.built, { items: [item] })) {
          findings.push({ id: value.built.id, signature: found.signature, status, reason: item.reason, size: minimal ? minimal.size : value.built.text.split("\n").length, repro: minimal?.dir ?? join(config.casesDir, value.built.id) });
        }
      }
    }
  }
  const groups = dedupe(findings.filter((f) => f.signature !== "-").map((f) => ({ ...f })));
  writeFileSync(join(out, "findings.json"), `${JSON.stringify({ generator: GENERATOR_VERSION, seed: options.seed, count: options.count, groups, refuted: findings.filter((f) => f.signature === "-") }, null, 2)}\n`);
  writeFileSync(join(out, "summary.json"), `${JSON.stringify({ programs: rows.length, verdicts: tally("verdict"), items: itemTally, candidates: candidates.length, confirmed, groups: groups.length }, null, 2)}\n`);
  for (const group of groups) console.error(`${group.count}x [${group.representative.status}] ${group.signature}\n    repro: ${group.representative.repro}`);

  if (confirmed) {
    console.error("\nSTOP: Chrome confirms a checker violation the real runtime contradicts. Do not continue the sweep; report the repro(s) above (scripts/fuzz/README.md).");
    return 3;
  }
  return 0;
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

/** Shrinks one candidate under the fast path (checker + Happy DOM); writes the minimal case. */
async function shrinkCandidate(value, config, options, out, { kind = "candidate-false-positive", code } = {}) {
  const target = value.classification.items.find((item) => item.kind === kind && (!code || item.diagnostic?.code === code));
  const targetSignature = signaturesOf(value.program, value.built, { items: [target] })[0]?.signature;
  let counter = 0;
  const base = `${value.built.id}-min`;
  const judge = async (program) => {
    counter += 1;
    const built = buildCase(program, { id: `${base}-${counter}` });
    const result = (await evaluate([{ program, built }], { ...config, casesDir: join(out, "shrink") })).get(built.id);
    const hit = result.classification.items.filter((item) => item.kind === kind);
    if (!hit.length) return false;
    if (options.shrinkMode === "discovery") return hit.some((item) => (kind === "candidate-false-clean" ? item.diagnostic.code === target.diagnostic.code : item.finding.rule === target.finding.rule));
    return signaturesOf(program, built, { items: hit }).some((found) => found.signature === targetSignature);
  };
  const cheap = (program) => [...walk(program)].some(({ node }) => isSite(node));
  const reduced = await shrink(value.program, judge, { budget: options.budget, cheap });
  const finalBuilt = buildCase(reduced.program, { id: `${value.built.id}-min`, note: `shrunk from fuzz seed ${value.program.seed} (${reduced.accepted} reductions, ${reduced.attempts} runs)` });
  const dir = writeCase(join(out, "repros", createHash("sha1").update(finalBuilt.text).digest("hex").slice(0, 10)), finalBuilt);
  console.error(`shrunk ${value.built.id}: ${reduced.attempts} runs, ${reduced.accepted} accepted, ${finalBuilt.text.split("\n").length} lines -> ${dir}`);
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
