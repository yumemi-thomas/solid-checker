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
import { DEFAULT_ORACLE_INSTALL, runChrome } from "./chrome.mjs";
import { classify } from "./classify.mjs";
import { dedupe } from "./dedupe.mjs";
import { buildCase, writeCase } from "./emit.mjs";
import { DEFAULT_FAST_INSTALL } from "./fast.mjs";
import { GENERATOR_VERSION, generateProgram } from "./generate.mjs";
import { evaluate, signaturesOf } from "./pipeline.mjs";
import { programSeed } from "./rng.mjs";
import { shrink } from "./shrink.mjs";
import { DEFAULT_TSC } from "./typecheck.mjs";
import { walk } from "./model.mjs";
import { recommendedConcurrency } from "../lib/pool.mjs";

const FLAGS = new Set(["confirm-with-chrome", "shrink", "keep-work", "generate-only", "help"]);
const VALUES = new Set(["seed", "count", "out", "profile", "install", "fast-install", "checker-bin", "typefacts", "tsc", "workers", "concurrency", "browser", "budget", "shrink-mode", "components"]);

export function parseArgs(argv) {
  const options = { seed: 1, count: 20, shrinkMode: "discovery", budget: 120 };
  for (let index = 0; index < argv.length; index += 1) {
    const raw = argv[index];
    if (!raw.startsWith("--")) throw new Error(`unexpected argument ${raw}`);
    const key = raw.slice(2);
    const camel = key.replace(/-([a-z])/g, (_, c) => c.toUpperCase());
    if (FLAGS.has(key)) { options[camel] = true; continue; }
    if (!VALUES.has(key)) throw new Error(`unknown option ${raw}`);
    if (argv[index + 1] === undefined) throw new Error(`${raw} needs a value`);
    index += 1;
    options[camel] = argv[index];
  }
  for (const key of ["seed", "count", "workers", "concurrency", "budget", "components"]) if (options[key] !== undefined) options[key] = Number(options[key]);
  for (const key of ["seed", "count", "budget"]) if (!Number.isInteger(options[key]) || options[key] < 0) throw new Error(`--${key} needs a non-negative integer`);
  if (!["discovery", "focused"].includes(options.shrinkMode)) throw new Error("--shrink-mode is discovery or focused");
  return options;
}

const usage = `usage: node scripts/fuzz/run.mjs --out <fresh dir> [--seed N] [--count N] [--profile name] [--components N]
  [--checker-bin <solid-checker>] [--typefacts <solid-typefacts>]   (or SOLID_CHECKER_NATIVE_BIN, SOLID_TYPEFACTS_BIN)
  [--install <oracle install>] [--fast-install <fuzz install>] [--tsc <tsrs>] [--workers N] [--concurrency N]
  [--confirm-with-chrome [--browser <chromium>]] [--shrink [--shrink-mode discovery|focused] [--budget N]]
  [--generate-only]   write and type-check the programs, run nothing else
  [--keep-work]       keep the materialized checker directories`;

async function main(options) {
  if (options.help) { console.log(usage); return 0; }
  if (!options.out) throw new Error(`--out <fresh directory> is required\n${usage}`);
  const out = resolve(options.out);
  if (existsSync(out)) throw new Error(`use a fresh --out path (${out} exists)`);
  mkdirSync(out, { recursive: true });

  const config = {
    install: resolve(options.install ?? DEFAULT_ORACLE_INSTALL),
    fastInstall: resolve(options.fastInstall ?? DEFAULT_FAST_INSTALL),
    bin: options.checkerBin ? resolve(options.checkerBin) : process.env.SOLID_CHECKER_NATIVE_BIN,
    typefacts: options.typefacts ? resolve(options.typefacts) : process.env.SOLID_TYPEFACTS_BIN,
    tsc: options.tsc ?? process.env.FUZZ_TSC ?? DEFAULT_TSC,
    casesDir: join(out, "cases"),
    concurrency: options.concurrency ?? recommendedConcurrency(),
    workers: options.workers ?? 4,
    keepWork: Boolean(options.keepWork),
  };
  if (!existsSync(join(config.install, "node_modules/solid-js/package.json"))) {
    throw new Error(`${config.install} has no node_modules/solid-js: run \`make runtime-oracle-install\``);
  }

  const entries = [];
  for (let index = 0; index < options.count; index += 1) {
    const seed = programSeed(options.seed, index);
    const program = generateProgram(seed, { profile: options.profile, components: options.components });
    entries.push({ program, built: buildCase(program, { id: `fuzz-${seed}` }) });
  }
  if (options.generateOnly) {
    for (const { built } of entries) writeCase(join(config.casesDir, built.id), built);
  }

  const results = options.generateOnly ? await typecheckOnly(entries, config) : await evaluate(entries, config);
  const rows = [];
  for (const [id, value] of results) {
    rows.push({
      id, seed: value.program.seed, verdict: value.classification.verdict, reasons: value.classification.reasons,
      items: value.classification.items, sites: value.built.sites.length, typeErrors: value.typecheck?.ok === false ? value.typecheck.errors : undefined,
      checkerMs: value.checker?.wallMs, runtimeMs: value.runtime?.wallMs,
    });
  }
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
    if (options.confirmWithChrome && candidates.length) {
      chrome = await runChrome(candidates.map((value) => ({ id: value.built.id, dir: join(config.casesDir, value.built.id), sites: value.built.sites })),
        { out, install: config.install, bin: config.bin, typefacts: config.typefacts, browser: options.browser, concurrency: options.concurrency });
    }
    for (const value of candidates) {
      const record = chrome.get(value.built.id);
      const judged = record ? classify({ sites: value.built.sites, checker: record.checker, runtime: record.runtime, plan: value.built.plan }) : null;
      const status = !record ? "unconfirmed" : judged.verdict === "candidate-false-positive" ? "confirmed" : "refuted-by-chrome";
      if (status === "confirmed") confirmed += 1;
      let minimal = null;
      if (options.shrink && status !== "refuted-by-chrome") minimal = await shrinkCandidate(value, config, options, out);
      for (const found of signaturesOf(value.program, value.built, judged ?? value.classification)) {
        findings.push({ id: value.built.id, signature: found.signature, status, size: minimal ? minimal.size : value.built.text.split("\n").length, rule: found.item.finding?.rule, reason: found.item.reason, repro: minimal?.dir ?? join(config.casesDir, value.built.id) });
      }
      if (judged && status === "refuted-by-chrome") findings.push({ id: value.built.id, signature: "-", status, reason: judged.reasons.join("; ") });
    }
    // Phase 4 hook: false cleans are classified and reported, never shrunk or confirmed in v1.
    for (const value of results.values()) {
      for (const found of signaturesOf(value.program, value.built, { items: value.classification.items.filter((item) => item.kind === "candidate-false-clean") })) {
        findings.push({ id: value.built.id, signature: found.signature, status: "false-clean-provisional", reason: found.item.reason, size: value.built.text.split("\n").length, repro: join(config.casesDir, value.built.id) });
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
async function shrinkCandidate(value, config, options, out) {
  const target = value.classification.items.find((item) => item.kind === "candidate-false-positive");
  const targetSignature = signaturesOf(value.program, value.built, value.classification)[0]?.signature;
  let counter = 0;
  const base = `${value.built.id}-min`;
  const judge = async (program) => {
    counter += 1;
    const built = buildCase(program, { id: `${base}-${counter}` });
    const result = (await evaluate([{ program, built }], { ...config, casesDir: join(out, "shrink") })).get(built.id);
    const hit = result.classification.items.filter((item) => item.kind === "candidate-false-positive");
    if (!hit.length) return false;
    if (options.shrinkMode === "discovery") return hit.some((item) => item.finding.rule === target.finding.rule);
    return signaturesOf(program, built, result.classification).some((found) => found.signature === targetSignature);
  };
  const cheap = (program) => [...walk(program)].some(({ node }) => (node.k === "read" || node.k === "write") && node.site);
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
