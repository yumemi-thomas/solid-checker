#!/usr/bin/env node
// The two numbers the checker is steered by (ADR 0328), from a corpus sweep
// and, optionally, a runtime-oracle report:
//
//   node scripts/corpus-metrics.mjs <sweep.json> [--oracle <oracle.json>] [--json]
//     [--manifest <manifest.json>] [--ledger <reviewed-findings.json>] [--mutants <report.json>]
//   node scripts/corpus-metrics.mjs --check-ledger [--ledger <reviewed-findings.json>]
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
// The share is also printed per split (dev / holdout, scripts/corpus-sweep.mjs
// --split): a row's split is its own `holdout` flag, else the manifest's.
//
// Reviewed precision (scripts/corpus/reviewed-findings.json, an independent
// classification): confirmed / adjudicated violations, from ledger entries
// that classify a violation of this sweep. Unreviewed violations are counted
// apart and never enter the ratio; an unresolved-review entry is not a
// decision; an entry whose project ran but whose key is gone is reported as
// stale, unless its `fixedBy` names the ADR that removed that false positive:
// then it is counted as fixed, and printed as REGRESSED if it comes back. --check-ledger validates the ledger file and exits 1 if malformed.
//
// Confirmed misses, beside it: confirmed-violation entries whose finding this
// sweep has only as uncertifiable (withheld) or not at all (absent), per rule,
// with the unresolved-review entries in scope. Entries of source `mutants` are
// read against the seeded probe's report (scripts/mutants.mjs, --mutants).
//
// Unknowns by blocker: the uncertifiable findings whose site no fact places
// ("no compiler or semantic fact places where ...", and the placement
// demotions), counted by the `blocker` each carries
// (`solid_reactive_ir::PlacementBlocker`), overall and per split, with the
// three apps that have the most of each. A report that predates the field
// counts such findings as `unrecorded`.
import { readFileSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import {
  bucketOf, confirmedMisses, decidedShare, holdoutIndex, reviewedPrecision, shareBySplit, topApps, unknownsByBlocker, validateLedger,
} from "./corpus-lib.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const argv = process.argv.slice(2);
const valueOptions = ["--oracle", "--manifest", "--ledger", "--mutants"];
const optionValue = name => (argv.includes(name) ? argv[argv.indexOf(name) + 1] : undefined);
const ledgerPath = resolve(optionValue("--ledger") ?? join(here, "corpus/reviewed-findings.json"));
const manifestPath = resolve(optionValue("--manifest") ?? join(here, "corpus/rc17-manifest.json"));

const ledger = JSON.parse(readFileSync(ledgerPath, "utf8"));
if (argv.includes("--check-ledger")) {
  const errors = validateLedger(ledger);
  for (const error of errors) console.error(`${ledgerPath}: ${error}`);
  if (!errors.length) console.log(`${ledgerPath}: ${ledger.entries.length} entries, well formed`);
  process.exit(errors.length ? 1 : 0);
}
const ledgerErrors = validateLedger(ledger);
if (ledgerErrors.length) throw new Error(`malformed ledger ${ledgerPath}: ${ledgerErrors.join("; ")}`);

const sweepPath = argv.find((argument, index) => !argument.startsWith("--") && !valueOptions.includes(argv[index - 1]));
if (!sweepPath) throw new Error("usage: corpus-metrics.mjs <sweep.json> [--oracle <oracle.json>] [--json] | --check-ledger");
const oraclePath = optionValue("--oracle");
const asJson = argv.includes("--json");

const sweep = JSON.parse(readFileSync(sweepPath, "utf8"));
const index = holdoutIndex(JSON.parse(readFileSync(manifestPath, "utf8")));

function category(path) {
  const lower = path.toLowerCase();
  if (/(^|\/)(__tests__|tests?|e2e|spec)(\/|$)|\.(test|spec)\.[cm]?[jt]sx?$/.test(lower)) return "test";
  if (/(^|\/)(stories|storybook|examples?|demos?|playgrounds?|docs?)(\/|$)|\.stories\.[cm]?[jt]sx?$/.test(lower)) return "story";
  if (/(^|\/)[^/]*\.config\.[cm]?[jt]s$/.test(lower)) return "config";
  return "app";
}

const empty = () => ({ violation: 0, uncertifiable: 0, style: 0, notice: 0 });
const total = empty();
const byCategory = {};
const byApp = {};
for (const result of sweep.results) {
  for (const finding of result.findings ?? []) {
    const path = finding.primaryLocation?.path ?? "";
    const where = category(relative(result.root, path));
    const slot = bucketOf(finding);
    for (const bucket of [total, (byCategory[where] ??= empty()), (byApp[result.app] ??= empty())]) {
      bucket[slot] += 1;
    }
  }
}
const share = decidedShare;
const splitShares = shareBySplit(sweep.results, index);
const reviewed = reviewedPrecision(sweep.results, ledger, index);
const mutantsPath = optionValue("--mutants");
const blockers = unknownsByBlocker(sweep.results, index);
const misses = confirmedMisses(sweep.results, ledger, mutantsPath ? JSON.parse(readFileSync(mutantsPath, "utf8")) : null);

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

const metrics = {
  sweep: sweepPath,
  split: sweep.split ?? "unrecorded",
  total: { ...total, decidedShare: share(total) },
  bySplit: Object.fromEntries(
    ["dev", "holdout"].map(name => [name, { ...splitShares[name], decidedShare: share(splitShares[name]) }])
  ),
  reviewed,
  misses: { mutants: mutantsPath ?? null, ...misses },
  unknownsByBlocker: blockers,
  byCategory: Object.fromEntries(
    Object.entries(byCategory).sort().map(([name, bucket]) => [name, { ...bucket, decidedShare: share(bucket) }])
  ),
  byApp: Object.fromEntries(
    Object.entries(byApp)
      .sort((left, right) => right[1].uncertifiable - left[1].uncertifiable)
      .map(([name, bucket]) => [name, { ...bucket, decidedShare: share(bucket) }])
  ),
  ...(oracle ? { oracle } : {})
};

if (asJson) {
  process.stdout.write(`${JSON.stringify(metrics, null, 2)}\n`);
} else {
  const percent = value => (value === null ? "  n/a" : `${(value * 100).toFixed(1).padStart(5)}%`);
  const line = (name, bucket) =>
    `${name.padEnd(24)} ${percent(bucket.decidedShare)}  violation ${String(bucket.violation).padStart(5)}  ` +
    `uncertifiable ${String(bucket.uncertifiable).padStart(5)}  style ${String(bucket.style).padStart(4)}  notice ${bucket.notice}`;
  console.log("decided share (violation / (violation + uncertifiable))");
  console.log(line("total", metrics.total));
  console.log(`by split (sweep ran: ${metrics.split})`);
  for (const [name, bucket] of Object.entries(metrics.bySplit)) console.log(line(`  ${name}`, bucket));
  console.log("by category");
  for (const [name, bucket] of Object.entries(metrics.byCategory)) console.log(line(`  ${name}`, bucket));
  console.log("by app");
  for (const [name, bucket] of Object.entries(metrics.byApp)) console.log(line(`  ${name}`, bucket));
  const precision = value => (value === null ? "n/a" : `${(value * 100).toFixed(1)}%`);
  const reviewLine = (name, bucket) =>
    `${name.padEnd(24)} ${precision(bucket.precision).padStart(6)}  confirmed ${bucket.confirmed}/${bucket.adjudicated} adjudicated  ` +
    `unresolved ${bucket.unresolved}  unreviewed ${bucket.unreviewed}`;
  console.log("reviewed precision (confirmed / adjudicated violations; unreviewed are not counted in it)");
  console.log(reviewLine("total", reviewed.total));
  console.log(reviewLine("  dev", reviewed.dev));
  console.log(reviewLine("  holdout", reviewed.holdout));
  if (reviewed.otherKind) console.log(`ledger entries on non-violation findings (not counted): ${reviewed.otherKind}`);
  for (const key of reviewed.stale) console.log(`  stale ledger entry ${key}`);
  if (reviewed.fixed.length) console.log(`fixed false positives still absent: ${reviewed.fixed.length}`);
  for (const key of reviewed.regressed) console.log(`  REGRESSED fixed false positive ${key}`);
  const missLine = (name, bucket) =>
    `${name.padEnd(24)} confirmed misses ${bucket.withheld + bucket.absent} (withheld ${bucket.withheld}, absent ${bucket.absent})  unresolved ${bucket.unresolved}`;
  console.log(`confirmed misses (confirmed-violation entries not proven now; mutants entries ${mutantsPath ? `from ${mutantsPath}` : "not read: no --mutants"})`);
  console.log(missLine("total", misses.total));
  for (const [rule, bucket] of Object.entries(misses.byRule)) console.log(missLine(`  ${rule}`, bucket));
  const partOf = (count, of) => (of === 0 ? "  n/a" : `${((count / of) * 100).toFixed(1).padStart(5)}%`);
  console.log(
    `unknowns by blocker (unplaced ${blockers.total.unplaced} of ${blockers.total.uncertifiable} uncertifiable; ` +
    `dev ${blockers.dev.unplaced}/${blockers.dev.uncertifiable}, holdout ${blockers.holdout.unplaced}/${blockers.holdout.uncertifiable})`,
  );
  console.log(`  ${"blocker".padEnd(30)} ${"count".padStart(5)}  unplaced  uncertifiable  ${"dev".padStart(5)}  ${"holdout".padStart(7)}  top apps`);
  for (const [name, slot] of Object.entries(blockers.total.blockers)) {
    const apps = topApps(slot).map(([app, count]) => `${app} ${count}`).join(", ");
    console.log(
      `  ${name.padEnd(30)} ${String(slot.count).padStart(5)}    ${partOf(slot.count, blockers.total.unplaced)}         ${partOf(slot.count, blockers.total.uncertifiable)}  ` +
      `${String(blockers.dev.blockers[name]?.count ?? 0).padStart(5)}  ${String(blockers.holdout.blockers[name]?.count ?? 0).padStart(7)}  ${apps}`,
    );
  }
  if (oracle) {
    console.log(`oracle: ${oracle.cases} cases, unproven clean ${oracle.unprovenClean.length}, known open ${oracle.knownOpen.length}, static-only ${oracle.staticOnly.length}`);
    for (const site of oracle.unprovenClean) console.log(`  unproven-clean ${site}`);
    for (const site of oracle.knownOpen) console.log(`  known-open ${site}`);
    for (const id of oracle.staticOnly) console.log(`  static-only ${id}`);
  }
}
