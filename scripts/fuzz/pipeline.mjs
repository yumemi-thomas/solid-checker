// One program's trip through the differential: type-check (tsrs), checker verdicts,
// fast runtime records, classification. Used for a whole batch (a run) and for a single
// program (the shrinker's predicate).
import { mkdirSync, rmSync } from "node:fs";
import { join } from "node:path";
import { mapPoolSettled } from "../lib/pool.mjs";
import { materializeForChecker, runChecker, siteVerdicts } from "./checker.mjs";
import { classify } from "./classify.mjs";
import { buildCase, writeCase } from "./emit.mjs";
import { runFast } from "./fast.mjs";
import { signature as findingSignature } from "./dedupe.mjs";
import { typecheck } from "./typecheck.mjs";

/**
 * Evaluates built cases (from `buildCase`) through the fast path.
 * `config`: { install (oracle install, hosts checker + type-check work dirs), fastInstall, bin, typefacts, tsc,
 *   casesDir (where case dirs are written), concurrency, workers, keepWork }
 * @returns {Promise<Map<string, object>>} id -> { program, built, typecheck, checker, runtime, classification, verdicts }
 */
export async function evaluate(entries, config, deps = {}) {
  const { runChecker: check = runChecker, runFast: fast = runFast, typecheck: types = typecheck } = deps;
  const results = new Map();
  const { install, casesDir } = config;
  mkdirSync(casesDir, { recursive: true });

  // 1. Write the case, type-check it as the oracle compiles it, and ask the checker.
  const prepared = await mapPoolSettled(entries, async ({ program, built }) => {
    const caseDir = writeCase(join(casesDir, built.id), built);
    const dir = materializeForChecker(built, install, built.id);
    const typed = await types(dir, { tsc: config.tsc });
    if (!typed.ok) return { program, built, caseDir, dir, typecheck: typed };
    const checker = await check(dir, { bin: config.bin, typefacts: config.typefacts });
    return { program, built, caseDir, dir, typecheck: typed, checker };
  }, { concurrency: config.concurrency });

  const live = [];
  for (const [index, settled] of prepared.entries()) {
    const { program, built } = entries[index];
    if (settled.error) { results.set(built.id, { program, built, error: String(settled.error.message ?? settled.error) }); continue; }
    results.set(built.id, { program, built, ...settled.value });
    if (settled.value.typecheck.ok && settled.value.checker?.ok) live.push(settled.value);
    if (!config.keepWork) rmSync(settled.value.dir, { recursive: true, force: true });
  }

  // 2. Run everything the checker answered in one Vitest batch.
  const records = live.length
    ? await fast(live.map(({ built, caseDir }) => ({ id: built.id, dir: caseDir })), { install: config.fastInstall, workers: config.workers })
    : new Map();

  // 3. Classify.
  for (const value of results.values()) {
    if (value.error || !value.typecheck?.ok) {
      value.classification = { verdict: "type-error", reasons: [value.error ?? value.typecheck?.errors?.[0] ?? "type error"], items: [] };
      continue;
    }
    if (!value.checker?.ok) { value.classification = classify({ sites: value.built.sites, checker: value.checker ?? { ok: false, error: "no checker run" }, runtime: { diagnostics: [], errors: [], witnesses: [] }, plan: value.built.plan }); continue; }
    value.runtime = records.get(value.built.id);
    value.verdicts = siteVerdicts(value.built.sites, value.checker.findings);
    value.classification = classify({ sites: value.built.sites, checker: value.checker, runtime: value.runtime, plan: value.built.plan });
  }
  return results;
}

/** Signatures of a classified result's candidate items: { signature, item } per candidate. */
export function signaturesOf(program, built, classification) {
  const out = [];
  for (const item of classification.items) {
    if (item.kind === "candidate-false-positive") {
      const site = built.sites.find((s) => s.id === item.siteIds?.[0]);
      if (site) out.push({ item, site, signature: findingSignature({ rule: item.finding.rule, kind: item.kind, program, path: site.path, outcome: "silent" }) });
    } else if (item.kind === "candidate-false-clean") {
      const site = built.sites.find((s) => s.line === item.diagnostic.line) ?? built.sites.reduce((best, s) => (best && Math.abs(best.line - item.diagnostic.line) <= Math.abs(s.line - item.diagnostic.line) ? best : s), null);
      if (site) out.push({ item, site, signature: findingSignature({ rule: item.diagnostic.code, kind: item.kind, program, path: site.path, outcome: "diagnosed" }) });
    }
  }
  return out;
}

export { buildCase };
