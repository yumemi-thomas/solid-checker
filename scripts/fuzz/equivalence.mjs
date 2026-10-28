#!/usr/bin/env node
// Equivalence mode of the differential fuzzer (metamorphic testing). See scripts/fuzz/README.md, "Equivalence mode".
//
//   node scripts/fuzz/equivalence.mjs --out <fresh dir> --seed 1 --count 200 --profile closure-escape --equivalence 3 \
//     --checker-bin <solid-checker> --typefacts <solid-typefacts> --tsc <tsrs> [--confirm-with-chrome] [--shrink]
//
// For each generated program the checker runs on the program and on N variants, each the program under one rewrite that does not
// change what it does at run time (rewrite.mjs). The verdict at every site of interest is compared across the two; the site map is the
// tag a site node carries through the rewrite. No runtime is needed to find a divergence: the rewrites are equivalent by construction,
// so a checker that answers differently on the two has a placement path that misses a guard. Chrome is only asked which side is right.
//
// A divergence is typed `equivalence-divergence` and sits in a bucket:
//   high       a violation on one side and nothing on the other, or two different rules: a user-visible difference.
//   certainty  (`certainty-divergence`) uncertifiable on one side and clean on the other. The forms are equivalent, so the clean side
//              claims a proof the checker cannot make on the other: a possible silent clean (fail-closed). `cleanSide` says which.
//   low        a violation on one side and uncertifiable on the other, or uncertifiable against another rule: internal (ADR 0328),
//              reported apart because the checker declining on one form is not what a user sees.
// Exit status: 0 nothing to stop for (divergences are findings, not failures), 3 Chrome shows a violation side the runtime contradicts
// (a false positive: stop and report), 5 a rewrite changed what Chrome runs (a fuzzer bug), 2 a usage or setup error.
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { mapPoolSettled } from "../lib/pool.mjs";
import { runChrome } from "./chrome.mjs";
import { assertFindingsMapped, materializeForChecker, runChecker, siteVerdicts } from "./checker.mjs";
import { disturbances, runtimeEvents } from "./classify.mjs";
import { codesForRule, isDefectCode } from "./codes.mjs";
import { hitCount } from "./hits.mjs";
import { isSite, walk } from "./model.mjs";
import { shrink } from "./shrink.mjs";
import { dedupe, signature as findingSignature } from "./dedupe.mjs";
import { buildCase, writeCase } from "./emit.mjs";
import { GENERATOR_VERSION } from "./generate.mjs";
import { getAt } from "./model.mjs";
import { hasPackages } from "./pipeline.mjs";
import { REWRITE_KINDS, SITE_TAG, applicableKinds, rewriteProgram } from "./rewrite.mjs";
import { createRng } from "./rng.mjs";
import { configFrom, generateEntries, parseArgs } from "./run.mjs";
import { requireTypeChecker, resolveTsc, typeErrorRefusal, typecheck } from "./typecheck.mjs";

/** The buckets, most severe first. */
export const BUCKETS = ["high", "certainty-divergence", "low"];

/** What the checker says at one site, reduced to what is compared: the verdict and the rules behind it. */
export function siteSummary(verdict) {
  const findings = verdict.findings ?? [];
  const violating = findings.filter((finding) => finding.kind === "violation");
  const rules = [...new Set((verdict.verdict === "violation" ? violating : findings).map((finding) => finding.rule))].sort();
  return { verdict: verdict.verdict, rules };
}

/**
 * Whether two summaries of the same site differ, and how. Returns null when they agree, else `{ bucket, cls, direction }`.
 * `direction` says which side is more severe: `original` (the original proves more) or `variant`.
 */
export function classifyDivergence(original, variant) {
  const same = original.verdict === variant.verdict && original.rules.join() === variant.rules.join();
  if (same) return null;
  const pair = [original.verdict, variant.verdict].sort().join("/");
  const order = { violation: 2, uncertifiable: 1, none: 0 };
  const direction = order[original.verdict] === order[variant.verdict] ? "same-verdict" : order[original.verdict] > order[variant.verdict] ? "original" : "variant";
  if (original.verdict === "violation" && variant.verdict === "violation") return { bucket: "high", cls: "rule-code", direction };
  if (pair === "none/violation") return { bucket: "high", cls: "violation-vs-clean", direction };
  if (pair === "uncertifiable/violation") return { bucket: "low", cls: "violation-vs-uncertifiable", direction };
  if (pair === "none/uncertifiable") return { bucket: "certainty-divergence", cls: "uncertifiable-vs-clean", direction, cleanSide: original.verdict === "none" ? "original" : "variant" };
  return { bucket: "low", cls: "uncertifiable-rule", direction };
}

/** The variant's site ids against the original's: `Map<variant site id, original site id>` from the tag every site node carries. */
export function siteMap(variantProgram, variantSites) {
  const map = new Map();
  for (const site of variantSites) {
    const tag = getAt(variantProgram, site.path)?.[SITE_TAG];
    if (tag) map.set(site.id, tag);
  }
  return map;
}

/**
 * Compares per-site verdicts of an original and one variant: `[{ siteId, line, variantLine, original, variant, bucket, cls, direction }]`.
 * A site with no counterpart (the map lost it) is an error of the rewrite, reported as `lost`.
 */
export function compareVariant({ originalSites, originalVerdicts, variantSites, variantVerdicts, map }) {
  const byOriginal = new Map(variantVerdicts.map((verdict) => [map.get(verdict.siteId), verdict]));
  const out = [];
  const lost = [];
  for (const verdict of originalVerdicts) {
    const other = byOriginal.get(verdict.siteId);
    if (!other) { lost.push(verdict.siteId); continue; }
    const a = siteSummary(verdict);
    const b = siteSummary(other);
    const divergence = classifyDivergence(a, b);
    if (divergence) out.push({ siteId: verdict.siteId, line: verdict.line, variantLine: other.line, original: a, variant: b, ...divergence });
  }
  return { divergences: out, lost, sites: originalSites.length };
}

/** The variants planned for a program: `n` of `{ kind, index, seed }`, cycling through the kinds that apply, in an order drawn from the program's seed. */
export function planVariants(program, n, { kinds = REWRITE_KINDS } = {}) {
  const rng = createRng((program.seed ^ 0xeb1e) >>> 0);
  const applicable = applicableKinds(program, kinds);
  const order = [];
  const pool = [...applicable];
  while (pool.length) order.push(pool.splice(Math.floor(rng.next() * pool.length), 1)[0]);
  return order.length ? Array.from({ length: n }, (_, index) => ({ kind: order[index % order.length], index, seed: ((program.seed ^ 0xeb1e) + (index + 1) * 7919) >>> 0 })) : [];
}

/** A subset of the candidates, drawn from the variant's seed: each with probability one half, at least one. */
export const subsetChooser = (seed) => (all) => {
  const rng = createRng(seed);
  const picked = all.filter(() => rng.chance(0.5));
  return picked.length || !all.length ? picked : [rng.pick(all)];
};

/** A program's checker run: type check, then the checker on the clean text. */
async function judge(built, config, deps, label) {
  const { install } = config;
  const check = deps.runChecker ?? runChecker;
  const types = deps.typecheck ?? typecheck;
  const dir = materializeForChecker(built, install, label);
  try {
    const typed = await types(dir, { tsc: config.tsc });
    if (!typed.ok) return { status: "type-error", errors: typed.errors };
    const checker = await check(dir, { bin: config.bin, typefacts: config.typefacts, ordinary: hasPackages(built) });
    if (!checker.ok) return { status: "checker-failed", error: checker.error };
    return { status: "ok", checker, verdicts: siteVerdicts(built.sites, checker.findings) };
  } finally {
    if (!config.keepWork) rmSync(dir, { recursive: true, force: true });
  }
}

/**
 * Runs the equivalence comparison for `entries` (`{ program, built }`), `n` variants each.
 * @returns {Promise<{ programs: object[], divergences: object[], typeErrors: object[], applied: object }>}
 */
export async function runEquivalence(entries, config, { n = 3, kinds = REWRITE_KINDS, out = null } = {}, deps = {}) {
  const settled = await mapPoolSettled(entries, async ({ program, built }) => {
    const row = { id: built.id, seed: program.seed, status: "ok", variants: [] };
    const original = await judge(built, config, deps, built.id);
    if (original.status !== "ok") return { row: { ...row, status: original.status }, divergences: [] };
    const divergences = [];
    for (const plan of planVariants(program, n, { kinds })) {
      const rewritten = rewriteProgram(program, plan.kind, { choose: subsetChooser(plan.seed) });
      const info = { kind: plan.kind, index: plan.index };
      if (!rewritten) { row.variants.push({ ...info, status: "no-candidates" }); continue; }
      const variantId = `${built.id}-eq${plan.index}-${plan.kind}`;
      const variantBuilt = buildCase(rewritten.program, { id: variantId });
      const answer = await judge(variantBuilt, config, deps, variantId);
      if (answer.status !== "ok") { row.variants.push({ ...info, status: answer.status, applied: rewritten.applied.length, error: (answer.errors ?? [answer.error])[0] }); continue; }
      const map = siteMap(rewritten.program, variantBuilt.sites);
      const compared = compareVariant({ originalSites: built.sites, originalVerdicts: original.verdicts, variantSites: variantBuilt.sites, variantVerdicts: answer.verdicts, map });
      row.variants.push({ ...info, status: compared.lost.length ? "lost-sites" : "ok", applied: rewritten.applied.length, divergences: compared.divergences.length, lost: compared.lost.length || undefined });
      for (const found of compared.divergences) {
        const site = built.sites.find((candidate) => candidate.id === found.siteId);
        divergences.push({
          kind: "equivalence-divergence", id: built.id, seed: program.seed, rewrite: plan.kind, variantIndex: plan.index, variantId, applied: rewritten.applied, ...found,
          signature: divergenceSignature(program, site, plan.kind, found), size: built.text.split("\n").length,
          originalFindings: original.verdicts.find((v) => v.siteId === found.siteId).findings, variantFindings: answer.verdicts.find((v) => map.get(v.siteId) === found.siteId).findings,
          originalText: built.text, variantText: variantBuilt.text, originalProgram: program, variantProgram: rewritten.program,
        });
      }
    }
    return { row, divergences };
  }, { concurrency: config.concurrency });
  const programs = []; const divergences = [];
  settled.forEach((result, index) => {
    if (result.error) { programs.push({ id: entries[index].built.id, seed: entries[index].program.seed, status: "error", error: String(result.error.message ?? result.error) }); return; }
    programs.push(result.value.row);
    divergences.push(...result.value.divergences);
  });
  const variants = programs.flatMap((row) => row.variants ?? []);
  const tally = (key) => variants.reduce((counts, variant) => ({ ...counts, [variant[key]]: (counts[variant[key]] ?? 0) + 1 }), {});
  const byKind = {};
  for (const variant of variants) {
    const slot = (byKind[variant.kind] ??= { variants: 0, ok: 0, typeError: 0, divergences: 0 });
    slot.variants += 1;
    if (variant.status === "ok") slot.ok += 1;
    if (variant.status === "type-error") slot.typeError += 1;
    slot.divergences += variant.divergences ?? 0;
  }
  return { programs, divergences, variants: tally("status"), byKind };
}

/** The fine signature of a divergence: the rewrite, the rule(s) on each side, and the construct path of the original site. */
export function divergenceSignature(program, site, rewrite, found) {
  const side = (summary) => `${summary.verdict}:${summary.rules.join("+") || "-"}`;
  return `${rewrite}|${findingSignature({ rule: found.original.rules[0] ?? found.variant.rules[0] ?? "-", kind: "equivalence-divergence", program, path: site.path, outcome: `${side(found.original)}->${side(found.variant)}` })}`;
}

/** Groups divergences by signature, per bucket, smallest program first. */
export function groupDivergences(divergences) {
  const groups = { high: [], "certainty-divergence": [], low: [] };
  for (const bucket of Object.keys(groups)) {
    groups[bucket] = dedupe(divergences.filter((d) => d.bucket === bucket).map((d) => ({ ...d, size: d.size }))).map((group) => ({
      signature: group.signature, count: group.count, rewrite: group.representative.rewrite, cls: group.representative.cls, direction: group.representative.direction,
      cleanSide: group.representative.cleanSide, original: group.representative.original, variant: group.representative.variant, representative: group.representative.variantId, members: group.members,
    }));
  }
  return groups;
}


// ---------------------------------------------------------------------------
// Chrome: which side of a divergence matches the runtime

const spansOf = (verdict) => (verdict.findings ?? []).filter((finding) => finding.kind === "violation").map((finding) => [finding.line, finding.endLine]);

/**
 * What Chrome's run of one side shows at one site: the codes of the defect diagnostics at the site's line (or inside the lines of
 * its violating findings), whether the site ran, and what disturbs the run.
 */
export function runtimeAtSite(built, record, siteId, verdict) {
  const site = built.sites.find((candidate) => candidate.id === siteId);
  const spans = [[site.line, site.line], ...spansOf(verdict)];
  const inside = (line) => spans.some(([from, to]) => from <= line && line <= to);
  const events = runtimeEvents(record.runtime);
  const codes = [...new Set(events.filter((event) => isDefectCode(event.code) && event.positions.some((position) => inside(position.line))).map((event) => event.code))].sort();
  const witness = record.runtime.witnesses?.find((candidate) => candidate.siteId === siteId);
  const ran = witness?.reached === true || hitCount(record.runtime.hits ?? null, site.hit) > 0;
  return { codes, ran, disturbed: disturbances(record.runtime, built.plan), line: site.line };
}

/** Whether two Chrome records show the same run: the same diagnostics and thrown codes, and the same witness at each mapped site. */
export function sameRun(original, variant, map) {
  const codes = (record) => runtimeEvents(record.runtime).map((event) => `${event.source}:${event.code}`).sort();
  const witnesses = (record, rename) => Object.fromEntries((record.runtime.witnesses ?? []).map((w) => [rename(w.siteId), w.actual ?? null]));
  const a = JSON.stringify({ codes: codes(original), witnesses: witnesses(original, (id) => id) });
  const b = JSON.stringify({ codes: codes(variant), witnesses: witnesses(variant, (id) => map.get(id) ?? id) });
  return { same: a === b, original: a, variant: b };
}

/**
 * Judges one divergence from the two Chrome records.
 * `status`: `violation-confirmed` (Chrome reports a defect the violating rule claims: the other side misses it), `violation-contradicted`
 * (the site ran, undisturbed, with no such diagnostic: the violation side is a false positive, exit 3), `defect-at-site` (certainty
 * divergence where Chrome reports a defect), `silent-at-site`, `inconclusive`, or `unsound-rewrite` (the two runs differ).
 * One direction only for a partial run (the driver failed, so part of the program never ran): a diagnostic reached before the failure can
 * confirm a violation or show a defect, but a missing diagnostic never yields `violation-contradicted`, `unsound-rewrite` or `silent-at-site`.
 */
export function judgeDivergence(divergence, originalBuilt, variantBuilt, originalRecord, variantRecord, map) {
  const run = sameRun(originalRecord, variantRecord, map);
  // A run the driver could not finish (a throw halts reactivity and the next step finds no element) still shows the diagnostics it
  // reached; but two cut-short runs may stop at different places, so a difference there is not evidence against the rewrite.
  const partial = Boolean(originalRecord.runtime.failure || variantRecord.runtime.failure);
  if (!run.same) return partial ? { status: "chrome-inconclusive", why: "the runs differ after a driver failure" } : { status: "unsound-rewrite", detail: { original: run.original, variant: run.variant } };
  const variantSite = [...map].find(([, original]) => original === divergence.siteId)?.[0];
  const originalVerdict = { findings: divergence.originalFindings };
  const variantVerdict = { findings: divergence.variantFindings };
  const at = { original: runtimeAtSite(originalBuilt, originalRecord, divergence.siteId, originalVerdict), variant: runtimeAtSite(variantBuilt, variantRecord, variantSite, variantVerdict) };
  const out = { at };
  // A partial run can confirm (a diagnostic it reached is evidence) but never refute: the part that never ran could hold the diagnostic.
  if (divergence.bucket === "certainty-divergence") return { ...out, status: at.original.codes.length || at.variant.codes.length ? "defect-at-site" : partial ? "inconclusive" : "silent-at-site", ...(partial && !at.original.codes.length && !at.variant.codes.length ? { why: "partial-run" } : {}) };
  const violating = [["original", divergence.original], ["variant", divergence.variant]].filter(([, summary]) => summary.verdict === "violation");
  const claimed = (rules) => rules.flatMap(codesForRule);
  const sides = violating.map(([side, summary]) => ({ side, confirmed: at[side].codes.some((code) => claimed(summary.rules).includes(code)), at: at[side] }));
  if (sides.every((entry) => entry.confirmed)) return { ...out, status: "violation-confirmed", sides: sides.map((entry) => entry.side) };
  const contradicted = sides.filter((entry) => !entry.confirmed && entry.at.ran && !entry.at.disturbed.length && !at.original.codes.length && !at.variant.codes.length);
  if (contradicted.length) return { ...out, status: "violation-contradicted", sides: contradicted.map((entry) => entry.side) };
  return { ...out, status: "inconclusive", why: partial ? "partial-run" : sides.some((entry) => !entry.at.ran) ? "site-not-run" : at.original.disturbed[0] ?? at.variant.disturbed[0] ?? "other-code" };
}

/** Runs one representative of each high and certainty group in Chrome (original and variant together) and judges it. */
export async function confirmInChrome(divergences, groups, config, options, out) {
  const reps = [];
  for (const bucket of ["high", "certainty-divergence"]) {
    for (const group of groups[bucket]) {
      const found = divergences.find((candidate) => candidate.signature === group.signature && candidate.variantId === group.representative);
      if (found) reps.push(found);
    }
  }
  const limit = Number(options.chromeMax ?? 40);
  const chosen = reps.slice(0, limit);
  if (!chosen.length) return [];
  const cases = [];
  const built = new Map();
  for (const found of chosen) {
    const originalBuilt = buildCase(found.originalProgram, { id: found.id });
    const variantBuilt = buildCase(found.variantProgram, { id: found.variantId });
    built.set(found.variantId, { originalBuilt, variantBuilt });
    for (const b of [originalBuilt, variantBuilt]) {
      if (cases.some((c) => c.id === b.id)) continue;
      cases.push({ id: b.id, dir: writeCase(join(out, "cases", b.id), b), sites: b.sites });
    }
  }
  const records = await runChrome(cases, { out, install: config.install, bin: config.bin, typefacts: config.typefacts, browser: options.browser, concurrency: options.concurrency, withClean: true });
  return chosen.map((found) => {
    const { originalBuilt, variantBuilt } = built.get(found.variantId);
    const originalRecord = records.get(found.id);
    const variantRecord = records.get(found.variantId);
    const map = siteMap(found.variantProgram, variantBuilt.sites);
    const missing = !originalRecord || !variantRecord || originalRecord.row === null || variantRecord.row === null;
    const judgement = missing ? { status: "chrome-inconclusive", why: "no oracle row" } : judgeDivergence(found, originalBuilt, variantBuilt, originalRecord, variantRecord, map);
    return { signature: found.signature, variantId: found.variantId, id: found.id, rewrite: found.rewrite, bucket: found.bucket, cls: found.cls, site: found.siteId, original: found.original, variant: found.variant, ...judgement };
  });
}


// ---------------------------------------------------------------------------
// Shrinking

/** Whether `a` and `b` are the same kind of divergence: the bucket, the class and the verdicts and rules on each side. */
const sameDivergence = (a, b) => a.bucket === b.bucket && a.cls === b.cls && JSON.stringify([a.original, a.variant]) === JSON.stringify([b.original, b.variant]);

/**
 * Reduces the original program of a divergence while the same divergence holds. The variant is the rewrite applied at every place it fits
 * (a function of the program, so the search has no choices to carry), and the predicate is "some site shows the same kind of divergence".
 * Returns the smallest program found, its two texts and the divergence.
 */
export async function shrinkDivergence(found, config, { budget = 80 } = {}, deps = {}) {
  let counter = 0;
  const probe = async (program) => {
    counter += 1;
    const rewritten = rewriteProgram(program, found.rewrite);
    if (!rewritten) return null;
    const originalBuilt = buildCase(program, { id: `${found.id}-sh${counter}` });
    const variantBuilt = buildCase(rewritten.program, { id: `${found.id}-sh${counter}-v` });
    const [original, variant] = [await judge(originalBuilt, config, deps, originalBuilt.id), await judge(variantBuilt, config, deps, variantBuilt.id)];
    if (original.status !== "ok" || variant.status !== "ok") return null;
    const compared = compareVariant({ originalSites: originalBuilt.sites, originalVerdicts: original.verdicts, variantSites: variantBuilt.sites, variantVerdicts: variant.verdicts, map: siteMap(rewritten.program, variantBuilt.sites) });
    const hit = compared.divergences.find((candidate) => sameDivergence(candidate, found));
    return hit ? { hit, originalBuilt, variantBuilt, rewritten } : null;
  };
  const start = await probe(found.originalProgram);
  if (!start) return null;
  const cheap = (program) => [...walk(program)].some(({ node }) => isSite(node));
  const reduced = await shrink(found.originalProgram, async (program) => Boolean(await probe(program)), { budget, cheap });
  const final = await probe(reduced.program);
  return final && { program: reduced.program, original: final.originalBuilt, variant: final.variantBuilt, divergence: final.hit, attempts: reduced.attempts, accepted: reduced.accepted };
}

/** Shrinks one representative of each high and certainty group and writes `repros/<hash>/`. */
export async function shrinkGroups(divergences, groups, config, options, out) {
  const reps = [];
  for (const bucket of ["high", "certainty-divergence"]) for (const group of groups[bucket]) {
    const found = divergences.find((candidate) => candidate.signature === group.signature && candidate.variantId === group.representative);
    if (found) reps.push(found);
  }
  const results = await mapPoolSettled(reps.slice(0, Number(options.shrinkMax ?? 20)), async (found) => {
    const minimal = await shrinkDivergence(found, { ...config, concurrency: 1 }, { budget: options.budget }, {});
    if (!minimal) return { signature: found.signature, shrunk: false };
    const dir = join(out, "repros", digest(minimal.original.text + minimal.variant.text));
    mkdirSync(dir, { recursive: true });
    writeFileSync(join(dir, "original.App.tsx"), minimal.original.text);
    writeFileSync(join(dir, "variant.App.tsx"), minimal.variant.text);
    const { site, ...rest } = { site: minimal.divergence.siteId, ...minimal.divergence };
    writeFileSync(join(dir, "divergence.json"), `${JSON.stringify({ rewrite: found.rewrite, site, ...rest }, null, 2)}\n`);
    return { signature: found.signature, shrunk: true, dir, lines: minimal.original.text.split("\n").length, attempts: minimal.attempts, accepted: minimal.accepted };
  }, { concurrency: Math.max(1, Math.min(4, config.concurrency)) });
  return results.map((result, index) => result.value ?? { signature: reps[index].signature, shrunk: false, error: String(result.error?.message ?? result.error) });
}

// ---------------------------------------------------------------------------
// Command line

const usage = `usage: node scripts/fuzz/equivalence.mjs --out <fresh dir> [--seed N] [--count N] [--profile name] [--equivalence N]
  [--rewrites a,b,c] [--exclude-rewrites a,b]   the rewrite kinds to draw (default all; see rewrite.mjs)
  [--checker-bin <solid-checker>] [--typefacts <solid-typefacts>] [--install <oracle install>] [--tsc <tsrs>] [--concurrency N]
  [--confirm-with-chrome [--browser <chromium>]]   Chrome says which side of a high-bucket divergence matches the runtime
  [--shrink [--budget N] [--shrink-max N]]   reduce one program per high and certainty group (the rewrite applied at every place it fits)
  [--keep-work]`;

export async function main(options) {
  if (options.help) { console.log(usage); return 0; }
  if (!options.out) throw new Error(`--out <fresh directory> is required\n${usage}`);
  const out = resolve(options.out);
  if (existsSync(out)) throw new Error(`use a fresh --out path (${out} exists)`);
  requireTypeChecker(resolveTsc(options.tsc));
  mkdirSync(out, { recursive: true });
  const config = configFrom(options, out);
  if (!existsSync(join(config.install, "node_modules/solid-js/package.json"))) throw new Error(`${config.install} has no node_modules/solid-js: run \`make runtime-oracle-install\``);
  const n = Number(options.equivalence ?? 3);
  if (!Number.isInteger(n) || n < 1) throw new Error("--equivalence needs a positive integer (variants per program)");
  const kinds = selectKinds(options);
  const entries = generateEntries(options);
  const result = await runEquivalence(entries, config, { n, kinds, out });
  if (entries.length && !result.programs.some((row) => row.status === "ok")) {
    throw new Error(`invalid run: no program was checked (${JSON.stringify(result.programs.reduce((counts, row) => ({ ...counts, [row.status]: (counts[row.status] ?? 0) + 1 }), {}))}; first error: ${result.programs[0]?.error ?? "see results.jsonl"})`);
  }
  assertFindingsMapped();
  const typeRefusal = typeErrorRefusal(result.programs.filter((row) => row.status === "type-error").length, result.programs.length);
  if (typeRefusal) throw new Error(typeRefusal);
  const groups = groupDivergences(result.divergences);
  writeDivergences(out, result.divergences, groups);
  const shrunk = options.shrink ? await shrinkGroups(result.divergences, groups, config, options, out) : [];
  const confirmations = options.confirmWithChrome ? await confirmInChrome(result.divergences, groups, config, options, out) : [];
  const byStatus = confirmations.reduce((counts, row) => ({ ...counts, [row.status]: (counts[row.status] ?? 0) + 1 }), {});
  const summary = {
    generator: GENERATOR_VERSION, seed: options.seed, count: options.count, profile: options.profile ?? null, variantsPerProgram: n, rewrites: kinds,
    programs: result.programs.length, programStatus: result.programs.reduce((counts, row) => ({ ...counts, [row.status]: (counts[row.status] ?? 0) + 1 }), {}),
    variants: result.variants, byRewrite: result.byKind,
    divergences: Object.fromEntries(BUCKETS.map((bucket) => [bucket, result.divergences.filter((d) => d.bucket === bucket).length])),
    groups: Object.fromEntries(BUCKETS.map((bucket) => [bucket, groups[bucket].length])),
    ...(options.confirmWithChrome ? { chrome: byStatus } : {}),
  };
  writeFileSync(join(out, "equivalence.json"), `${JSON.stringify({ ...summary, ...groups, confirmations, shrunk }, null, 2)}\n`);
  writeFileSync(join(out, "results.jsonl"), `${result.programs.map((row) => JSON.stringify(row)).join("\n")}\n`);
  console.error(`programs ${summary.programs} ${JSON.stringify(summary.programStatus)}; variants ${JSON.stringify(summary.variants)}`);
  console.error(`divergences: ${BUCKETS.map((bucket) => `${bucket} ${summary.divergences[bucket]} in ${summary.groups[bucket]} group(s)`).join(", ")}`);
  for (const group of [...groups.high, ...groups["certainty-divergence"]]) console.error(`${group.cleanSide ? `certainty (${group.cleanSide} clean)` : "high"} ${group.count}x [${group.rewrite}] ${group.cls} ${group.original.verdict}:${group.original.rules.join("+") || "-"} -> ${group.variant.verdict}:${group.variant.rules.join("+") || "-"}\n    ${group.signature}\n    repro: ${join(out, "divergences", reproName(group.representative))}`);
  for (const row of shrunk) console.error(`shrink ${row.shrunk ? `${row.lines} lines -> ${row.dir}` : `not reduced${row.error ? ` (${row.error})` : ""}`}`);
  for (const row of confirmations) console.error(`chrome [${row.status}] ${row.bucket} ${row.rewrite} ${row.variantId}${row.why ? ` (${row.why})` : ""}${row.sides ? ` sides ${row.sides.join(",")}` : ""}`);
  if (byStatus["violation-contradicted"]) {
    console.error("\nSTOP: Chrome shows a site the checker proves a violation at running with no diagnostic, while an equivalent form is judged otherwise (a false positive). Report the repro(s) above.");
    return 3;
  }
  if (byStatus["unsound-rewrite"]) {
    console.error("\nFUZZER BUG: a rewrite changed what Chrome runs. Drop the rewrite (--exclude-rewrites) and report it.");
    return 5;
  }
  return 0;
}

const reproName = (variantId) => variantId;

/** The kinds a run draws: `--rewrites` names them, `--exclude-rewrites` removes some. */
export function selectKinds(options) {
  const named = options.rewrites ? String(options.rewrites).split(",").filter(Boolean) : REWRITE_KINDS;
  const dropped = new Set(String(options.excludeRewrites ?? "").split(",").filter(Boolean));
  for (const kind of [...named, ...dropped]) if (!REWRITE_KINDS.includes(kind)) throw new Error(`unknown rewrite ${kind} (known: ${REWRITE_KINDS.join(", ")})`);
  return named.filter((kind) => !dropped.has(kind));
}

/** Every divergence as `divergences/<variant id>/{original.App.tsx, variant.App.tsx, divergence.json}` (one directory per variant), and the jsonl. */
function writeDivergences(out, divergences, groups) {
  const root = join(out, "divergences");
  mkdirSync(root, { recursive: true });
  const seen = new Map();
  for (const divergence of divergences) {
    const dir = join(root, divergence.variantId);
    mkdirSync(dir, { recursive: true });
    if (!seen.has(divergence.variantId)) {
      writeFileSync(join(dir, "original.App.tsx"), divergence.originalText);
      writeFileSync(join(dir, "variant.App.tsx"), divergence.variantText);
      writeFileSync(join(dir, "variant.program.json"), `${JSON.stringify(divergence.variantProgram)}\n`);
      writeFileSync(join(dir, "original.program.json"), `${JSON.stringify(divergence.originalProgram)}\n`);
      seen.set(divergence.variantId, []);
    }
    seen.get(divergence.variantId).push({ site: divergence.siteId, line: divergence.line, variantLine: divergence.variantLine, bucket: divergence.bucket, cls: divergence.cls, direction: divergence.direction, cleanSide: divergence.cleanSide, rewrite: divergence.rewrite, applied: divergence.applied, original: divergence.original, variant: divergence.variant, originalFindings: divergence.originalFindings, variantFindings: divergence.variantFindings, signature: divergence.signature });
  }
  for (const [id, rows] of seen) writeFileSync(join(root, id, "divergence.json"), `${JSON.stringify(rows, null, 2)}\n`);
  writeFileSync(join(out, "divergences.jsonl"), divergences.map(({ originalText, variantText, originalProgram, variantProgram, ...row }) => JSON.stringify(row)).join("\n") + (divergences.length ? "\n" : ""));
  return groups;
}

export const digest = (text) => createHash("sha1").update(text).digest("hex").slice(0, 10);

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    process.exitCode = await main(parseArgs(process.argv.slice(2), { extraValues: ["equivalence", "rewrites", "exclude-rewrites", "chrome-max", "shrink-max"] }));
  } catch (error) {
    console.error(`equivalence: ${error.message}`);
    process.exitCode = 2;
  }
}
