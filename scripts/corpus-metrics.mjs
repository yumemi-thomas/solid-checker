#!/usr/bin/env node
// The two numbers the checker is steered by (ADR 0328), from a corpus sweep
// and, optionally, a runtime-oracle report:
//
//   node scripts/corpus-metrics.mjs <sweep.json> [--oracle <oracle.json>] [--json]
//
// 1. Unproven clean claims: runtime diagnostics the checker is silent on, from
//    the oracle report (`unproven-clean`, plus `known-open` debt). Target 0.
// 2. Decided share: violations / (violations + uncertifiable), overall, by
//    code category, and by app. Uncertifiable results are internal, so this
//    share, not the raw uncertifiable count, says how much of what the
//    analysis met it could decide. Style rules (SC80xx) and project notices
//    (`subjectKind: "project"`) are reported apart and left out of the share.
//
// A finding's category is read from its path, relative to the project root:
// `test` (tests, specs, __tests__, e2e), `story` (stories, examples, demos,
// playgrounds, docs sites), `config` (*.config.*), and `app` for the rest.
// The heuristic is coarse and stated; it only splits the share, it never
// drops a finding.
//
// Uncertifiable results are counted twice, side by side: as rows (every
// finding, the number this report always gave) and as SITES: one for each
// distinct (app, project, path, span) that holds at least one uncertifiable
// row. A place several rules answer with an obligation (a read that SC5001,
// SC5003 and SC9012 each report) is one site, and its other rows are ECHO
// rows: `uncertifiableSites + echoRows = uncertifiable`. The site count is the
// number of places the analysis could not decide; the row count and the share
// by rows stay as they were, and `decidedShareBySite` is the share with the
// sites in its denominator. Two projects that analyze the same file are two
// analyses: a site is counted once in each. Nothing here reads a finding
// differently: no finding, judge or checker moves.
import { readFileSync } from "node:fs";
import { relative } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

/** A finding's category, from its path relative to the project root. */
export function category(path) {
  const lower = path.toLowerCase();
  if (/(^|\/)(__tests__|tests?|e2e|spec)(\/|$)|\.(test|spec)\.[cm]?[jt]sx?$/.test(lower)) return "test";
  if (/(^|\/)(stories|storybook|examples?|demos?|playgrounds?|docs?)(\/|$)|\.stories\.[cm]?[jt]sx?$/.test(lower)) return "story";
  if (/(^|\/)[^/]*\.config\.[cm]?[jt]s$/.test(lower)) return "config";
  return "app";
}

const empty = () => ({ violation: 0, uncertifiable: 0, uncertifiableSites: 0, echoRows: 0, style: 0, notice: 0 });

/** Violations over violations plus `denominator` (the uncertifiable rows, or the uncertifiable sites): none when nothing was decided or left open. */
const shareOf = (bucket, uncertifiable) => {
  const decided = bucket.violation + uncertifiable;
  return decided === 0 ? null : bucket.violation / decided;
};

/**
 * The buckets of a sweep: `{ total, byCategory, byApp }`, each `{ violation, uncertifiable (rows), uncertifiableSites, echoRows, style, notice }`.
 * A finding is a notice when it is about the project, a style finding when its id is an SC80xx, a violation when its kind says so, else an uncertifiable row;
 * the first uncertifiable row at a (app, project, path, start, end) is its site, every later one at the same place an echo row.
 */
export function bucketsOf(sweep) {
  const total = empty();
  const byCategory = {};
  const byApp = {};
  const sites = new Set();
  for (const result of sweep.results) {
    for (const finding of result.findings ?? []) {
      const path = finding.primaryLocation?.path ?? "";
      const where = category(relative(result.root, path));
      const slot =
        finding.subjectKind === "project" ? "notice"
          : /^SC80/.test(finding.id) ? "style"
            : finding.kind === "violation" ? "violation" : "uncertifiable";
      const buckets = [total, (byCategory[where] ??= empty()), (byApp[result.app] ??= empty())];
      for (const bucket of buckets) bucket[slot] += 1;
      if (slot === "uncertifiable") {
        const key = `${result.app}\0${result.project}\0${path}\0${finding.primaryLocation?.startByte ?? 0}\0${finding.primaryLocation?.endByte ?? 0}`;
        const field = sites.has(key) ? "echoRows" : "uncertifiableSites";
        sites.add(key);
        for (const bucket of buckets) bucket[field] += 1;
      }
    }
  }
  return { total, byCategory, byApp };
}

/** A bucket with its two shares: by rows (the report's own) and by sites. */
const withShares = (bucket) => ({ ...bucket, decidedShare: shareOf(bucket, bucket.uncertifiable), decidedShareBySite: shareOf(bucket, bucket.uncertifiableSites) });

/** The report of a sweep: the buckets with their shares, the by-category and by-app maps sorted as the report prints them. */
export function metricsOf(sweep, sweepPath = "", oracle = undefined) {
  const { total, byCategory, byApp } = bucketsOf(sweep);
  return {
    sweep: sweepPath,
    total: withShares(total),
    byCategory: Object.fromEntries(Object.entries(byCategory).sort().map(([name, bucket]) => [name, withShares(bucket)])),
    byApp: Object.fromEntries(
      Object.entries(byApp)
        .sort((left, right) => right[1].uncertifiable - left[1].uncertifiable)
        .map(([name, bucket]) => [name, withShares(bucket)]),
    ),
    ...(oracle ? { oracle } : {}),
  };
}

/** The text of the report (without the oracle's lines). */
export function renderMetrics(metrics) {
  const percent = (value) => (value === null ? "  n/a" : `${(value * 100).toFixed(1).padStart(5)}%`);
  const line = (name, bucket) =>
    `${name.padEnd(24)} ${percent(bucket.decidedShare)}  violation ${String(bucket.violation).padStart(5)}  ` +
    `uncertifiable ${String(bucket.uncertifiable).padStart(5)} (${bucket.uncertifiableSites} sites, ${bucket.echoRows} echo; ${percent(bucket.decidedShareBySite).trim()} by site)  ` +
    `style ${String(bucket.style).padStart(4)}  notice ${bucket.notice}`;
  const lines = ["decided share (violation / (violation + uncertifiable)), by rows; the by-site share counts each site once"];
  lines.push(line("total", metrics.total));
  for (const [name, bucket] of Object.entries(metrics.byCategory)) lines.push(line(`  ${name}`, bucket));
  lines.push("by app");
  for (const [name, bucket] of Object.entries(metrics.byApp)) lines.push(line(`  ${name}`, bucket));
  return lines;
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  const argv = process.argv.slice(2);
  const sweepPath = argv.find(argument => !argument.startsWith("--") && argv[argv.indexOf(argument) - 1] !== "--oracle");
  if (!sweepPath) throw new Error("usage: corpus-metrics.mjs <sweep.json> [--oracle <oracle.json>] [--json]");
  const oracleIndex = argv.indexOf("--oracle");
  const oraclePath = oracleIndex >= 0 ? argv[oracleIndex + 1] : undefined;
  const asJson = argv.includes("--json");

  const sweep = JSON.parse(readFileSync(sweepPath, "utf8"));

  let oracle;
  if (oraclePath) {
    const report = JSON.parse(readFileSync(oraclePath, "utf8"));
    const sites = verdict => report.results.flatMap(result =>
      result.diagnostics.filter(diagnostic => diagnostic.verdict === verdict)
        .map(diagnostic => `${result.id} ${diagnostic.code} ${diagnostic.file}:${diagnostic.line}`));
    oracle = {
      cases: report.cases,
      unprovenClean: sites("unproven-clean"),
      knownOpen: sites("known-open"),
      staticOnly: report.results.filter(result => result.verdict === "static-only").map(result => result.id)
    };
  }

  const metrics = metricsOf(sweep, sweepPath, oracle);

  if (asJson) {
    process.stdout.write(`${JSON.stringify(metrics, null, 2)}\n`);
  } else {
    for (const line of renderMetrics(metrics)) console.log(line);
    if (oracle) {
      console.log(`oracle: ${oracle.cases} cases, unproven clean ${oracle.unprovenClean.length}, known open ${oracle.knownOpen.length}, static-only ${oracle.staticOnly.length}`);
      for (const site of oracle.unprovenClean) console.log(`  unproven-clean ${site}`);
      for (const site of oracle.knownOpen) console.log(`  known-open ${site}`);
      for (const id of oracle.staticOnly) console.log(`  static-only ${id}`);
    }
  }
}
