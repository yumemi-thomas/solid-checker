// scripts/corpus-metrics.mjs: uncertifiable results as rows and as sites (an obligation several rules report at one place is one site, the other rows its echo), and nothing else moved.
import assert from "node:assert/strict";
import { relative } from "node:path";
import { test } from "vitest";

import { bucketsOf, category, metricsOf, renderMetrics } from "./corpus-metrics.mjs";

const at = (path, start, end = start + 5) => ({ path, startByte: start, endByte: end, line: 1, column: 1 });
const finding = (id, kind, path, start, extra = {}) => ({ id, kind, primaryLocation: at(path, start), ...extra });
const U = (id, path, start) => finding(id, "uncertifiable", path, start);
const V = (id, path, start) => finding(id, "violation", path, start);

const sweep = () => ({
  results: [
    {
      app: "a", project: "tsconfig.json", root: "/p",
      findings: [
        // one read answered by three rules: one site, two echo rows
        U("SC5001", "/p/src/App.tsx", 10), U("SC5003", "/p/src/App.tsx", 10), U("SC9012", "/p/src/App.tsx", 10),
        // the same rule twice at one place, and another place
        U("SC1001", "/p/src/Other.tsx", 30), U("SC1001", "/p/src/Other.tsx", 30), U("SC2001", "/p/src/Other.tsx", 50),
        // a test file's obligation and a violation, a style finding, a project notice
        U("SC9012", "/p/src/App.test.tsx", 7), V("SC1001", "/p/src/App.tsx", 90), finding("SC8015", "uncertifiable", "/p/src/App.tsx", 100),
        { id: "SC9013", kind: "violation", subjectKind: "project", primaryLocation: at("/p/tsconfig.json", 0) },
      ],
    },
    {
      // another project of the same app analyzes the same file: its own analysis, its own site
      app: "a", project: "tsconfig.node.json", root: "/p",
      findings: [U("SC5001", "/p/src/App.tsx", 10)],
    },
    { app: "b", project: "tsconfig.json", root: "/q", findings: [U("SC4001", "/q/x.ts", 1), U("SC9012", "/q/x.ts", 1)] },
  ],
});

test("an obligation several rules report at one place is one site and the other rows are echo rows; the rows are all still counted", () => {
  const { total, byApp, byCategory } = bucketsOf(sweep());
  // rows: 3 + 3 + 1 + 1 + 2 = 10 (the style finding is not one); sites: App.tsx@10, Other.tsx@30, Other.tsx@50, App.test.tsx@7, App.tsx@10 of the other project, x.ts@1 = 6
  assert.equal(total.uncertifiable, 10);
  assert.equal(total.uncertifiableSites, 6);
  assert.equal(total.echoRows, 4);
  assert.equal(total.uncertifiableSites + total.echoRows, total.uncertifiable);
  assert.deepEqual([total.violation, total.style, total.notice], [1, 1, 1]);
  assert.deepEqual(
    Object.entries(byApp).map(([name, bucket]) => [name, bucket.uncertifiable, bucket.uncertifiableSites, bucket.echoRows]),
    [["a", 8, 5, 3], ["b", 2, 1, 1]],
  );
  assert.deepEqual(
    Object.entries(byCategory).map(([name, bucket]) => [name, bucket.uncertifiable, bucket.uncertifiableSites, bucket.echoRows]).sort(),
    [["app", 9, 5, 4], ["test", 1, 1, 0]],
  );
  // every bucket adds up, whatever the split
  for (const bucket of [total, ...Object.values(byApp), ...Object.values(byCategory)]) assert.equal(bucket.uncertifiableSites + bucket.echoRows, bucket.uncertifiable);
});

test("the shares: by rows as the report always gave it, and by sites", () => {
  const { total } = metricsOf(sweep());
  assert.equal(total.decidedShare, 1 / (1 + 10));
  assert.equal(total.decidedShareBySite, 1 / (1 + 6));
  const none = metricsOf({ results: [{ app: "a", project: "p", root: "/p", findings: [] }] });
  assert.equal(none.total.decidedShare, null);
  assert.equal(none.total.decidedShareBySite, null);
});

/** The report's counting before sites were added, kept here as the reference: rows by slot per bucket. */
function referenceRows(report) {
  const make = () => ({ violation: 0, uncertifiable: 0, style: 0, notice: 0 });
  const total = make();
  const byCategory = {};
  const byApp = {};
  for (const result of report.results) {
    for (const found of result.findings ?? []) {
      const where = category(relative(result.root, found.primaryLocation?.path ?? ""));
      const slot = found.subjectKind === "project" ? "notice" : /^SC80/.test(found.id) ? "style" : found.kind === "violation" ? "violation" : "uncertifiable";
      for (const bucket of [total, (byCategory[where] ??= make()), (byApp[result.app] ??= make())]) bucket[slot] += 1;
    }
  }
  return { total, byCategory, byApp };
}

test("no row, category or app count moved: every row count and the share by rows equal the earlier counting", () => {
  const report = sweep();
  const { total, byCategory, byApp } = bucketsOf(report);
  const reference = referenceRows(report);
  const rows = (bucket) => ({ violation: bucket.violation, uncertifiable: bucket.uncertifiable, style: bucket.style, notice: bucket.notice });
  assert.deepEqual(rows(total), reference.total);
  assert.deepEqual(Object.fromEntries(Object.entries(byCategory).map(([name, bucket]) => [name, rows(bucket)])), reference.byCategory);
  assert.deepEqual(Object.fromEntries(Object.entries(byApp).map(([name, bucket]) => [name, rows(bucket)])), reference.byApp);
  const metrics = metricsOf(report);
  assert.equal(metrics.total.decidedShare, reference.total.violation / (reference.total.violation + reference.total.uncertifiable));
  // the apps stay in the order of their uncertifiable rows, the categories sorted by name
  assert.deepEqual(Object.keys(metrics.byApp), ["a", "b"]);
  assert.deepEqual(Object.keys(metrics.byCategory), ["app", "test"]);
});

test("the text names the sites and the echo rows next to the rows", () => {
  const lines = renderMetrics(metricsOf(sweep()));
  const total = lines.find((line) => line.startsWith("total"));
  assert.match(total, /uncertifiable\s+10 \(6 sites, 4 echo; /);
  assert.match(lines[0], /by rows; the by-site share counts each site once/);
});
