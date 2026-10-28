#!/usr/bin/env node
// Replay: re-judge recorded programs with the current checker, without regenerating a seed. See scripts/fuzz/README.md, "Replay".
//
//   node scripts/fuzz/replay.mjs --out <fresh dir> <dir|list> [...] [--confirm-with-chrome [--no-chrome-cache] [--cache-dir <dir>]]
//     [--checker-bin <solid-checker>] [--typefacts <solid-typefacts>] [--tsc <tsrs>] [--install <oracle install>] [--concurrency N]
//
// An argument is a directory or a text file listing directories (one per line, `#` comments; a path may be followed by a label after
// whitespace). A directory is an item when it holds one of:
//   pair        `original.program.json` and `variant.program.json` (an equivalence divergence, `divergences/<variant id>/`): both trees are
//               rebuilt as cases, checked, compared at the recorded sites by the site map, and (with Chrome) judged as equivalence.mjs does
//   text pair   `original.App.tsx` and `variant.App.tsx` with `divergence.json` (a shrunk equivalence repro): both texts are checked and
//               compared at the recorded lines; Chrome runs each text with the oracle's own driver (no plan, no witnesses)
//   plain       `App.tsx` (a repro): the checker's findings against Chrome's defect diagnostics in the file
// Any other directory is searched for items below it.
//
// Output (`replay.json`, `replay.jsonl`, a line per item on stderr): the item's bucket now (`agree` when the two sides agree at every
// recorded site), the bucket recorded with it, Chrome's status and whether it came from the Chrome cache, and `fixed` when the two sides
// now agree on a violation and Chrome still shows the defect (a plain repro: every defect diagnostic Chrome reports is covered by a
// claiming violation). A cached Chrome result is named on its row (`chromeCache: "hit"`), so it never turns a disagreement into agreement
// unseen. Exit status: 0, or 3 when Chrome contradicts a violation (`violation-contradicted`: stop and report), 2 on a usage error.
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { basename, dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { mapPoolSettled } from "../lib/pool.mjs";
import { chromeCacheFor } from "./cache.mjs";
import { runChrome } from "./chrome.mjs";
import { siteVerdicts } from "./checker.mjs";
import { runtimeEvents } from "./classify.mjs";
import { codeRules, codesForRule, ignoredRules, isDefectCode, ruleHasRuntimeCounterpart, withholdingRules } from "./codes.mjs";
import { buildCase, writeCase } from "./emit.mjs";
import { classifyDivergence, compareVariant, judge, judgeDivergence, siteMap, siteSummary } from "./equivalence.mjs";
import { configFrom, parseArgs } from "./run.mjs";
import { requireTypeChecker, resolveTsc } from "./typecheck.mjs";

const isFile = (path) => existsSync(path) && statSync(path).isFile();
const readJson = (path) => JSON.parse(readFileSync(path, "utf8"));

/** The kind of a directory: `pair`, `text-pair`, `plain`, or null (not an item). */
export function itemKind(dir) {
  if (isFile(join(dir, "original.program.json")) && isFile(join(dir, "variant.program.json"))) return "pair";
  if (isFile(join(dir, "original.App.tsx")) && isFile(join(dir, "variant.App.tsx")) && isFile(join(dir, "divergence.json"))) return "text-pair";
  if (isFile(join(dir, "App.tsx"))) return "plain";
  return null;
}

/** The items named by the arguments: `{ dir, kind, label }`, in order, each directory once. */
export function collectItems(args) {
  const items = [];
  const seen = new Set();
  const visit = (path, label) => {
    const dir = resolve(path);
    if (seen.has(dir)) return;
    seen.add(dir);
    const kind = itemKind(dir);
    if (kind) { items.push({ dir, kind, label: label ?? null }); return; }
    for (const entry of readdirSync(dir, { withFileTypes: true }).sort((a, b) => (a.name < b.name ? -1 : 1))) {
      if (entry.isDirectory() && !["cases", "chrome-cases", "node_modules"].includes(entry.name) && !entry.name.startsWith(".")) visit(join(dir, entry.name));
    }
  };
  for (const arg of args) {
    if (!existsSync(arg)) throw new Error(`no such file or directory: ${arg}`);
    if (statSync(arg).isDirectory()) { visit(arg); continue; }
    for (const raw of readFileSync(arg, "utf8").split("\n")) {
      const line = raw.replace(/#.*$/, "").trim();
      if (!line) continue;
      const [path, ...rest] = line.split(/\s+/);
      if (!existsSync(path)) throw new Error(`${arg}: no such directory ${path}`);
      visit(path, rest.join(" ") || null);
    }
  }
  return items;
}

/** The original program's id of a divergence directory named by its variant id (`fuzz-N-eqK-kind`). */
export const originalId = (variantId) => variantId.replace(/-eq\d+-[a-z-]+$/, "");

/** What the run that wrote a divergence directory recorded about it: its rows (divergence.json) and Chrome's row (equivalence.json). */
export function recordedPair(dir) {
  const rows = [readJson(join(dir, "divergence.json"))].flat();
  const run = dirname(dirname(dir));
  const summary = isFile(join(run, "equivalence.json")) ? readJson(join(run, "equivalence.json")) : null;
  const chrome = summary?.confirmations?.filter((row) => row.variantId === basename(dir)) ?? [];
  return { rows, chrome };
}

/**
 * The verdict of findings at a line, as siteVerdicts gives it for a site there: `{ verdict, rules, findings }`.
 * For a text pair, whose sites are not known, the recorded line stands for the site.
 */
export function verdictAtLine(findings, line) {
  const [verdict] = siteVerdicts([{ id: "line", line, kind: "line" }], findings);
  return { ...siteSummary(verdict), findings: verdict.findings };
}

/** The defect codes Chrome's run shows at a line (or inside the given finding spans), and whether the run failed partway. */
export function chromeCodesAt(record, line, findings = []) {
  const spans = [[line, line], ...findings.filter((finding) => finding.kind === "violation").map((finding) => [finding.line, finding.endLine])];
  const inside = (at) => spans.some(([from, to]) => from <= at && at <= to);
  const appPositions = (event) => event.positions.filter((position) => position.file === "App.tsx" || position.file === undefined);
  const codes = runtimeEvents(record.runtime).filter((event) => isDefectCode(event.code) && appPositions(event).some((position) => inside(position.line))).map((event) => event.code);
  return { codes: [...new Set(codes)].sort(), partial: Boolean(record.runtime?.failure) };
}

/**
 * The status of an item now. `now` is the comparison of the two sides at each recorded site (`agree` or a bucket), `verdict` the agreed
 * verdict when they agree. `fixed` needs the two sides to agree on a violation whose rule claims a code Chrome reports at the site.
 */
export function fixedStatus({ now, verdict, rules, chromeCodes }) {
  if (now !== "agree") return false;
  if (verdict !== "violation") return false;
  const claimed = rules.flatMap(codesForRule);
  return chromeCodes.some((code) => claimed.includes(code));
}

// ---------------------------------------------------------------------------------------------------------------------
// The kinds

// Work directories are labelled by the item's index: two items may share an original program (two variants of one seed) or a basename.
async function replayPair(item, config, out) {
  const variantId = basename(item.dir);
  const id = originalId(variantId);
  const originalProgram = readJson(join(item.dir, "original.program.json"));
  const variantProgram = readJson(join(item.dir, "variant.program.json"));
  const originalBuilt = buildCase(originalProgram, { id });
  const variantBuilt = buildCase(variantProgram, { id: variantId });
  const recorded = recordedPair(item.dir);
  const [original, variant] = [await judge(originalBuilt, config, {}, `rp${item.index}-o`), await judge(variantBuilt, config, {}, `rp${item.index}-v`)];
  const base = { kind: item.kind, dir: item.dir, label: item.label, id, variantId, rewrite: recorded.rows[0]?.rewrite ?? null, recorded: { buckets: recorded.rows.map((row) => `${row.site}:${row.bucket}`), chrome: recorded.chrome.map((row) => row.status) } };
  if (original.status !== "ok" || variant.status !== "ok") return { ...base, now: "not-judged", why: `original ${original.status}, variant ${variant.status}` };
  const map = siteMap(variantProgram, variantBuilt.sites);
  const compared = compareVariant({ originalSites: originalBuilt.sites, originalVerdicts: original.verdicts, variantSites: variantBuilt.sites, variantVerdicts: variant.verdicts, map });
  const sites = recorded.rows.map((row) => {
    const found = compared.divergences.find((candidate) => candidate.siteId === row.site);
    const verdict = original.verdicts.find((candidate) => candidate.siteId === row.site);
    const variantVerdict = variant.verdicts.find((candidate) => map.get(candidate.siteId) === row.site);
    return { site: row.site, recorded: row.bucket, now: found ? found.bucket : verdict ? "agree" : "lost", original: verdict ? siteSummary(verdict) : null, variant: variantVerdict ? siteSummary(variantVerdict) : null, found, row, verdict, variantVerdict };
  });
  const now = worst(sites.map((site) => site.now));
  const value = { ...base, now, sites: sites.map(({ site, recorded: was, now: is, original: a, variant: b }) => ({ site, recorded: was, now: is, original: a, variant: b })) };
  return { ...value, chromeInput: { originalBuilt, variantBuilt, map, sites, originalProgram, variantProgram } };
}

async function replayTextPair(item, config) {
  const rows = [readJson(join(item.dir, "divergence.json"))].flat();
  const name = basename(item.dir);
  const originalBuilt = textCase(`${caseIdOf(item)}-o`, readFileSync(join(item.dir, "original.App.tsx"), "utf8"));
  const variantBuilt = textCase(`${caseIdOf(item)}-v`, readFileSync(join(item.dir, "variant.App.tsx"), "utf8"));
  const [original, variant] = [await judge(originalBuilt, config, {}, originalBuilt.id), await judge(variantBuilt, config, {}, variantBuilt.id)];
  const base = { kind: item.kind, dir: item.dir, label: item.label, rewrite: rows[0]?.rewrite ?? null, recorded: { buckets: rows.map((row) => `${row.site ?? row.siteId}:${row.bucket}`) } };
  if (original.status !== "ok" || variant.status !== "ok") return { ...base, now: "not-judged", why: `original ${original.status}, variant ${variant.status}` };
  const sites = rows.map((row) => {
    const a = verdictAtLine(original.checker.findings, row.line);
    const b = verdictAtLine(variant.checker.findings, row.variantLine ?? row.line);
    const divergence = classifyDivergence(a, b);
    return { site: row.site ?? row.siteId, line: row.line, variantLine: row.variantLine ?? row.line, recorded: row.bucket, now: divergence ? divergence.bucket : "agree", original: { verdict: a.verdict, rules: a.rules }, variant: { verdict: b.verdict, rules: b.rules }, findings: { original: a.findings, variant: b.findings } };
  });
  return { ...base, now: worst(sites.map((site) => site.now)), sites: sites.map(({ findings, ...site }) => site), chromeInput: { originalBuilt, variantBuilt, sites } };
}

async function replayPlain(item, config) {
  const built = textCase(caseIdOf(item), readFileSync(join(item.dir, "App.tsx"), "utf8"));
  const result = await judge(built, config, {}, built.id);
  const base = { kind: item.kind, dir: item.dir, label: item.label };
  if (result.status !== "ok") return { ...base, now: "not-judged", why: result.status };
  const findings = result.checker.findings.filter((finding) => !ignoredRules.includes(finding.rule));
  return { ...base, now: "checked", findings: findings.map((finding) => `${finding.rule}/${finding.kind}@${finding.line}`), chromeInput: { built, findings } };
}

/** A bare text's case id: its directory's name and a hash of its path (stable across lists, so the Chrome cache finds it; unique across directories). */
export const caseIdOf = (item) => `rp-${basename(item.dir).replace(/[^A-Za-z0-9_-]/g, "_").slice(0, 40)}-${createHash("sha1").update(item.dir).digest("hex").slice(0, 6)}`;

/** A case from a bare App.tsx: no sites, no plan, no twin; the oracle drives it with its own event passes. */
function textCase(id, text) {
  return { id, text, sites: [], glue: [], packages: {}, plan: { clicks: [] }, files: { "App.tsx": text } };
}

const ORDER = ["high", "certainty-divergence", "low", "lost", "agree"];
/** The most severe of the sites' states. */
const worst = (states) => [...states].sort((a, b) => ORDER.indexOf(a) - ORDER.indexOf(b))[0] ?? "agree";

// ---------------------------------------------------------------------------------------------------------------------
// Chrome

/** Judges each replayed item against its Chrome records; sets `chrome`, `chromeCache` and `fixed` on the row. */
export function judgeWithChrome(row, records) {
  const input = row.chromeInput;
  if (!input) return row;
  // `cached` is on a record only when the run had a Chrome cache.
  const cachedOf = (...ids) => (ids.some((id) => records.get(id)?.cached === true) ? "hit" : ids.some((id) => records.get(id)?.cached === false) ? "miss" : undefined);
  if (row.kind === "plain") {
    const record = records.get(input.built.id);
    if (!record?.row) return { ...row, chrome: { status: "chrome-inconclusive", why: "no oracle row" } };
    const events = runtimeEvents(record.runtime).filter((event) => isDefectCode(event.code));
    const judged = events.map((event) => {
      const lines = event.positions.filter((position) => position.file === "App.tsx" || position.file === undefined).map((position) => position.line);
      const rules = codeRules[event.code] ?? [];
      const covering = input.findings.filter((finding) => lines.some((line) => finding.line <= line && line <= finding.endLine));
      const claims = (finding) => rules.includes(finding.rule) || withholdingRules.includes(finding.rule);
      const status = covering.some((finding) => finding.kind === "violation" && rules.includes(finding.rule)) ? "agree"
        : covering.some((finding) => claims(finding)) ? "recall-gap"
          : !lines.length && input.findings.some((finding) => finding.kind === "violation" && rules.includes(finding.rule)) ? "agree" : "false-clean";
      return { code: event.code, line: lines[0] ?? null, status };
    });
    // A violation of a rule a runtime diagnostic can contradict, where Chrome reports no defect of its codes on its lines: a candidate
    // false positive. A bare text has no witness, so nothing shows the site ran: it is for a person to read, never confirmed here.
    const unmatched = input.findings.filter((finding) => finding.kind === "violation" && ruleHasRuntimeCounterpart(finding.rule)
      && !events.some((event) => codesForRule(finding.rule).includes(event.code)));
    const base = !judged.length ? (record.runtime.failure ? "chrome-inconclusive" : "no-defect") : judged.every((entry) => entry.status === "agree") ? "agree" : judged.some((entry) => entry.status === "false-clean") ? "false-clean" : "recall-gap";
    const status = unmatched.length && !record.runtime.failure && ["no-defect", "agree"].includes(base) ? "violation-without-diagnostic" : base;
    return { ...row, chrome: { status, diagnostics: judged, ...(unmatched.length ? { unmatched: unmatched.map((finding) => `${finding.rule}@${finding.line}`) } : {}), ...(record.runtime.failure ? { partial: true } : {}) },
      chromeCache: cachedOf(input.built.id), fixed: status === "agree" };
  }
  const originalRecord = records.get(input.originalBuilt.id);
  const variantRecord = records.get(input.variantBuilt.id);
  const chromeCache = cachedOf(input.originalBuilt.id, input.variantBuilt.id);
  if (!originalRecord?.row || !variantRecord?.row) return { ...row, chrome: { status: "chrome-inconclusive", why: "no oracle row" }, chromeCache, fixed: false };
  if (row.kind === "pair") {
    // The divergence to judge at each site: the one found now, else the recorded one (both sides agree now; Chrome says whether the defect stands).
    const judgements = input.sites.map((site) => {
      const divergence = site.found ?? { ...site.row, siteId: site.site, originalFindings: site.row.originalFindings, variantFindings: site.row.variantFindings };
      const judged = judgeDivergence(divergence, input.originalBuilt, input.variantBuilt, originalRecord, variantRecord, input.map);
      const codes = [...new Set([...(judged.at?.original.codes ?? []), ...(judged.at?.variant.codes ?? [])])];
      return { site: site.site, status: judged.status, why: judged.why, codes, sides: judged.sides };
    });
    const fixed = input.sites.every((site, index) => fixedStatus({ now: site.now, verdict: site.original?.verdict, rules: site.original?.rules ?? [], chromeCodes: judgements[index].codes }));
    return { ...row, chrome: { status: summarize(judgements.map((entry) => entry.status)), sites: judgements }, chromeCache, fixed };
  }
  // A text pair: the defect codes at the recorded line on each side. A bare text has no driver plan (the oracle's event passes only:
  // no button that disposes a host, reruns a memo or calls a handle), so Chrome's silence there is not evidence: inconclusive.
  const judgements = input.sites.map((site) => {
    const a = chromeCodesAt(originalRecord, site.line);
    const b = chromeCodesAt(variantRecord, site.variantLine);
    const codes = [...new Set([...a.codes, ...b.codes])].sort();
    const status = codes.length ? "defect-at-site" : "inconclusive";
    return { site: site.site, status, codes, ...(codes.length ? {} : { why: a.partial || b.partial ? "partial-run" : "no-driver-plan" }) };
  });
  const fixed = input.sites.every((site, index) => fixedStatus({ now: site.now, verdict: site.original.verdict, rules: site.original.rules, chromeCodes: judgements[index].codes }));
  return { ...row, chrome: { status: summarize(judgements.map((entry) => entry.status)), sites: judgements }, chromeCache, fixed };
}

/** One status for an item's sites: the first of the severe ones, else the first. */
const summarize = (statuses) => ["violation-contradicted", "unsound-rewrite", "violation-confirmed", "defect-at-site"].find((status) => statuses.includes(status)) ?? statuses[0] ?? "none";

async function confirm(rows, config, options, out) {
  const cases = [];
  const add = (built, twin) => {
    if (cases.some((entry) => entry.id === built.id)) return;
    cases.push({ id: built.id, dir: writeCase(join(out, "cases", built.id), built), sites: built.sites, twin });
  };
  for (const row of rows) {
    const input = row.chromeInput;
    if (!input) continue;
    if (row.kind === "plain") add(input.built, false);
    else { add(input.originalBuilt, row.kind === "pair"); add(input.variantBuilt, row.kind === "pair"); }
  }
  const settings = { out, install: config.install, bin: config.bin, typefacts: config.typefacts, browser: options.browser, concurrency: options.concurrency, cache: config.chromeCache };
  // A generated case runs its twin beside its clean control (as equivalence.mjs does); a bare text runs as it is.
  const twins = cases.filter((entry) => entry.twin);
  const plain = cases.filter((entry) => !entry.twin);
  const records = new Map([
    ...(twins.length ? await runChrome(twins, { ...settings, withClean: true }) : new Map()),
    ...(plain.length ? await runChrome(plain, { ...settings, control: true }) : new Map()),
  ]);
  return rows.map((row) => judgeWithChrome(row, records));
}

// ---------------------------------------------------------------------------------------------------------------------
// Command line

const usage = `usage: node scripts/fuzz/replay.mjs --out <fresh dir> <dir|list> [...]
  [--confirm-with-chrome [--browser <chromium>] [--no-chrome-cache] [--cache-dir <dir>]]
  [--checker-bin <solid-checker>] [--typefacts <solid-typefacts>] [--install <oracle install>] [--tsc <tsrs>] [--concurrency N] [--keep-work]`;

export async function main(argv) {
  const paths = [];
  const flags = [];
  for (let index = 0; index < argv.length; index += 1) {
    if (argv[index].startsWith("--")) {
      flags.push(argv[index]);
      if (!["--confirm-with-chrome", "--no-chrome-cache", "--keep-work", "--help", "--all-cores"].includes(argv[index]) && argv[index + 1] !== undefined) flags.push(argv[++index]);
    } else paths.push(argv[index]);
  }
  const options = parseArgs(flags, { extraValues: ["cache-dir"], extraFlags: ["no-chrome-cache"] });
  if (options.help) { console.log(usage); return 0; }
  if (!options.out || !paths.length) throw new Error(`--out <fresh directory> and at least one item are required\n${usage}`);
  const out = resolve(options.out);
  if (existsSync(out)) throw new Error(`use a fresh --out path (${out} exists)`);
  requireTypeChecker(resolveTsc(options.tsc));
  const items = collectItems(paths).map((item, index) => ({ ...item, index }));
  if (!items.length) throw new Error("no replayable item under the given paths");
  mkdirSync(out, { recursive: true });
  const base = configFrom(options, out);
  const config = options.confirmWithChrome ? { ...base, chromeCache: chromeCacheFor(base, options) } : base;
  const started = Date.now();
  const settled = await mapPoolSettled(items, async (item) => (item.kind === "pair" ? replayPair(item, config, out) : item.kind === "text-pair" ? replayTextPair(item, config) : replayPlain(item, config)), { concurrency: config.concurrency });
  let rows = settled.map((result, index) => result.value ?? { kind: items[index].kind, dir: items[index].dir, label: items[index].label, now: "error", why: String(result.error?.message ?? result.error) });
  const checkerMs = Date.now() - started;
  if (options.confirmWithChrome) rows = await confirm(rows, config, options, out);
  const report = rows.map(({ chromeInput, ...row }) => row);
  const summary = {
    items: report.length, checkerMs, wallMs: Date.now() - started,
    now: tally(report, (row) => row.now), fixed: report.filter((row) => row.fixed).length,
    ...(options.confirmWithChrome ? { chrome: tally(report, (row) => row.chrome?.status), chromeCache: config.chromeCache ? config.chromeCache.stats() : null } : {}),
  };
  writeFileSync(join(out, "replay.jsonl"), `${report.map((row) => JSON.stringify(row)).join("\n")}\n`);
  writeFileSync(join(out, "replay.json"), `${JSON.stringify({ summary, items: report }, null, 2)}\n`);
  for (const row of report) {
    const chrome = row.chrome ? ` chrome ${row.chrome.status}${row.chromeCache === "hit" ? " (cached)" : ""}` : "";
    const was = row.recorded?.buckets ? ` (recorded ${row.recorded.buckets.join(",")}${row.recorded.chrome?.length ? `; chrome ${row.recorded.chrome.join(",")}` : ""})` : "";
    console.error(`${row.fixed ? "FIXED " : ""}${row.now}${chrome} ${row.label ?? row.variantId ?? basename(row.dir)}${was}${row.why ? ` [${row.why}]` : ""}`);
  }
  console.error(`replay: ${JSON.stringify(summary)}`);
  return report.some((row) => row.chrome?.status === "violation-contradicted") ? 3 : 0;
}

const tally = (rows, keyOf) => rows.reduce((counts, row) => { const key = keyOf(row) ?? "-"; counts[key] = (counts[key] ?? 0) + 1; return counts; }, {});

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    process.exitCode = await main(process.argv.slice(2));
  } catch (error) {
    console.error(`replay: ${error.message}`);
    process.exitCode = 2;
  }
}
