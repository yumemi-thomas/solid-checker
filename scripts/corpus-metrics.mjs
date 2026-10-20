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
import { readFileSync } from "node:fs";
import { relative } from "node:path";

const argv = process.argv.slice(2);
const sweepPath = argv.find(argument => !argument.startsWith("--") && argv[argv.indexOf(argument) - 1] !== "--oracle");
if (!sweepPath) throw new Error("usage: corpus-metrics.mjs <sweep.json> [--oracle <oracle.json>] [--json]");
const oracleIndex = argv.indexOf("--oracle");
const oraclePath = oracleIndex >= 0 ? argv[oracleIndex + 1] : undefined;
const asJson = argv.includes("--json");

const sweep = JSON.parse(readFileSync(sweepPath, "utf8"));

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
    const slot =
      finding.subjectKind === "project" ? "notice"
        : /^SC80/.test(finding.id) ? "style"
          : finding.kind === "violation" ? "violation" : "uncertifiable";
    for (const bucket of [total, (byCategory[where] ??= empty()), (byApp[result.app] ??= empty())]) {
      bucket[slot] += 1;
    }
  }
}
const share = bucket => {
  const decided = bucket.violation + bucket.uncertifiable;
  return decided === 0 ? null : bucket.violation / decided;
};

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
  total: { ...total, decidedShare: share(total) },
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
  for (const [name, bucket] of Object.entries(metrics.byCategory)) console.log(line(`  ${name}`, bucket));
  console.log("by app");
  for (const [name, bucket] of Object.entries(metrics.byApp)) console.log(line(`  ${name}`, bucket));
  if (oracle) {
    console.log(`oracle: ${oracle.cases} cases, unproven clean ${oracle.unprovenClean.length}, known open ${oracle.knownOpen.length}, static-only ${oracle.staticOnly.length}`);
    for (const site of oracle.unprovenClean) console.log(`  unproven-clean ${site}`);
    for (const site of oracle.knownOpen) console.log(`  known-open ${site}`);
    for (const id of oracle.staticOnly) console.log(`  static-only ${id}`);
  }
}
