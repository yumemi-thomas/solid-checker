// One program's trip through the differential: type-check (tsrs), checker verdicts,
// fast runtime records, classification. Used for a whole batch (a run) and for a single
// program (the shrinker's predicate).
//
// The checker reads the clean text. The type check of the clean text is the authority for "tsc accepts
// this" (invariant 1); the instrumented twin (hits.mjs) is type-checked too, because it is what runs,
// and it runs in Happy DOM. The neutrality gate (hits.mjs, README): strip(runText) == text is checked
// when the case is built; `config.control` runs the clean twin in the same engine and compares the
// outcomes (and the checker on the twin's text); both are required before a dead-site verdict.
import { mkdirSync, rmSync } from "node:fs";
import { join } from "node:path";
import { mapPoolSettled } from "../lib/pool.mjs";
import { materializeForChecker, runChecker, siteVerdicts } from "./checker.mjs";
import { classify } from "./classify.mjs";
import { buildCase, writeCase } from "./emit.mjs";
import { runFast } from "./fast.mjs";
import { findingsDiff, outcomeDiff } from "./hits.mjs";
import { closedGrammar } from "./model.mjs";
import { signature as findingSignature } from "./dedupe.mjs";
import { callAtLine } from "./falseclean.mjs";
import { typecheck } from "./typecheck.mjs";

/** A case that ships a generated package is checked as a user's run is (checker.mjs ORDINARY_ARGS): the package is read from source. */
export const hasPackages = (built) => Object.keys(built.packages ?? {}).length > 0;

const inconclusive = (reason, why) => ({ verdict: "inconclusive", reasons: [reason], items: [{ kind: "inconclusive", reason, why }] });

/**
 * Evaluates built cases (from `buildCase`) through the fast path.
 * `config`: { install (oracle install, hosts checker + type-check work dirs), fastInstall, bin, typefacts, tsc,
 *   casesDir (where case dirs are written), concurrency, workers, keepWork,
 *   instrument (default true; false runs the clean text and classifies as before the hit ledger: the baseline of the neutrality regression),
 *   control (also run the clean twin and the checker on the twin's text, and compare),
 *   deadSite (enable the dead-site subkind; implies control) }
 * @returns {Promise<Map<string, object>>} id -> { program, built, typecheck, checker, runtime, classification, verdicts, control }
 */
export async function evaluate(entries, config, deps = {}) {
  const { runChecker: check = runChecker, typecheck: types = typecheck } = deps;
  // The long-lived host (`config.fast`) serves the batches when there is one; tests inject `deps.runFast`.
  const fast = deps.runFast ?? (config.fast ? (cases) => config.fast.run(cases) : runFast);
  const results = new Map();
  const { install, casesDir } = config;
  const instrument = config.instrument !== false;
  const control = instrument && Boolean(config.control || config.deadSite);
  mkdirSync(casesDir, { recursive: true });

  // 1. Write the case, type-check it as the oracle compiles it, and ask the checker (on the clean text).
  const prepared = await mapPoolSettled(entries, async ({ program, built }) => {
    const caseDir = writeCase(join(casesDir, built.id), built);
    const dir = materializeForChecker(built, install, built.id);
    const typed = await types(dir, { tsc: config.tsc });
    if (!typed.ok) return { program, built, caseDir, dir, typecheck: typed };
    const ordinary = hasPackages(built);
    const checker = await check(dir, { bin: config.bin, typefacts: config.typefacts, ordinary });
    const value = { program, built, caseDir, dir, typecheck: typed, checker };
    if (instrument) {
      const twinDir = materializeForChecker(built, install, `${built.id}-run`, { twin: true });
      value.twinTypecheck = await types(twinDir, { tsc: config.tsc });
      if (control && checker.ok && value.twinTypecheck.ok) value.twinChecker = await check(twinDir, { bin: config.bin, typefacts: config.typefacts, ordinary });
      if (!config.keepWork) rmSync(twinDir, { recursive: true, force: true });
    }
    return value;
  }, { concurrency: config.concurrency });

  const live = [];
  for (const [index, settled] of prepared.entries()) {
    const { program, built } = entries[index];
    if (settled.error) { results.set(built.id, { program, built, error: String(settled.error.message ?? settled.error) }); continue; }
    results.set(built.id, { program, built, ...settled.value });
    const value = settled.value;
    if (value.typecheck.ok && value.checker?.ok && (!instrument || (built.neutral && value.twinTypecheck?.ok))) live.push(value);
    if (!config.keepWork) rmSync(value.dir, { recursive: true, force: true });
  }

  // 2. Run everything the checker answered in one Vitest batch (the twin), and, as a control, the clean twin in a second.
  const variant = instrument ? "run" : "clean";
  const records = live.length
    ? await fast(live.map(({ built, caseDir }) => ({ id: built.id, dir: caseDir, variant })), { install: config.fastInstall, workers: config.workers })
    : new Map();
  const controls = control && live.length
    ? await fast(live.map(({ built, caseDir }) => ({ id: `${built.id}-clean`, dir: caseDir, variant: "clean" })), { install: config.fastInstall, workers: config.workers })
    : new Map();

  // 3. Classify.
  for (const value of results.values()) {
    if (value.error || !value.typecheck?.ok) {
      value.classification = { verdict: "type-error", reasons: [value.error ?? value.typecheck?.errors?.[0] ?? "type error"], items: [] };
      continue;
    }
    if (!value.checker?.ok) { value.classification = classify({ sites: value.built.sites, checker: value.checker ?? { ok: false, error: "no checker run" }, runtime: { diagnostics: [], errors: [], witnesses: [] }, plan: value.built.plan }); continue; }
    if (instrument && !value.built.neutral) { value.classification = inconclusive("instrumentation is not neutral: stripping the counters does not give the clean text", "not-neutral"); continue; }
    if (instrument && !value.twinTypecheck?.ok) { value.classification = inconclusive(`the instrumented twin does not type-check: ${value.twinTypecheck?.errors?.[0] ?? "type error"}`, "twin-type-error"); continue; }
    value.runtime = records.get(value.built.id);
    value.verdicts = siteVerdicts([...value.built.sites, ...(instrument ? value.built.glue : [])], value.checker.findings);
    let neutral = instrument;
    if (control) {
      const clean = controls.get(`${value.built.id}-clean`);
      const diffs = [...(clean ? outcomeDiff(value.runtime, clean) : ["no clean-twin record"]),
        ...(value.twinChecker?.ok ? findingsDiff(value.checker.findings, value.twinChecker.findings).map((d) => `checker ${d}`) : ["the checker failed on the instrumented text"])];
      value.control = { diffs, matched: diffs.length === 0, clean };
      neutral = diffs.length === 0;
      if (diffs.length) { value.classification = inconclusive(`instrumentation-perturbed-run: ${diffs[0]}`, "perturbed-run"); continue; }
    }
    value.classification = classify({
      sites: value.built.sites, glue: instrument ? value.built.glue : [], checker: value.checker, runtime: value.runtime, plan: value.built.plan,
      flags: { deadSite: Boolean(config.deadSite) },
      evidence: { ledger: value.built.ledger, closed: closedGrammar(value.program), neutral },
    });
  }
  return results;
}

/**
 * Chrome's judgement of a fast-path result: the same comparator, on Chrome's runtime and checker records
 * (the oracle's findings on the twin, compared with the fast pipeline's clean ones: a difference is
 * `instrumentation-perturbed-checker`). A dead-site candidate is confirmed only if Chrome itself meets every condition.
 */
export function classifyChrome(value, record, config = {}) {
  return classify({
    sites: value.built.sites, glue: value.built.glue ?? [], checker: record.checker, runtime: record.runtime, plan: value.built.plan,
    flags: { deadSite: Boolean(config.deadSite) },
    evidence: { ledger: value.built.ledger, closed: closedGrammar(value.program), neutral: Boolean(value.built.neutral) && !record.checker.perturbed?.length && value.control?.matched === true },
  });
}

/** Signatures of a classified result's candidate items: { signature, item } per candidate. */
export function signaturesOf(program, built, classification) {
  const out = [];
  const known = [...built.sites, ...(built.glue ?? [])];
  for (const item of classification.items) {
    if (item.kind === "candidate-false-positive") {
      const site = known.find((s) => s.id === item.siteIds?.[0]);
      if (site) out.push({ item, site, signature: findingSignature({ rule: item.finding.rule, kind: item.kind, program, path: site.path, outcome: item.subkind ?? "silent" }) });
    } else if (item.kind === "candidate-wrong-rule") {
      const site = known.find((candidate) => candidate.line === item.finding.line) ?? known.find((candidate) => Math.abs(candidate.line - item.finding.line) <= 1);
      if (site) out.push({ item, site, signature: findingSignature({ rule: item.finding.rule, kind: item.kind, program, path: site.path, outcome: item.diagnostic.code }) });
    } else if (item.kind === "candidate-false-clean") {
      // The node at the diagnostic's innermost App line, from the printer's line table (not the nearest site by line).
      const node = item.diagnostic.line === null ? null : callAtLine(built.lineTable, item.diagnostic.line);
      if (node) out.push({ item, site: built.sites.find((s) => s.line === item.diagnostic.line) ?? null, signature: findingSignature({ rule: item.diagnostic.code, kind: item.kind, program, path: node.path, outcome: "diagnosed" }) });
    }
  }
  return out;
}

export { buildCase };
