import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

import {
  bucketOf, confirmedMisses, findingKey, holdoutIndex, mutantSiteKey, reviewedPrecision, selectRows, shareBySplit, splitOf,
  validateLedger,
} from "./corpus-lib.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const manifest = JSON.parse(readFileSync(join(here, "corpus/rc17-manifest.json"), "utf8"));

const finding = (id, kind, path, start = 1, end = 2, extra = {}) => ({ id, kind, primaryLocation: { path, startByte: start, endByte: end }, ...extra });
const entry = (key, cls = "confirmed-violation", source = "corpus-sweep") => ({ key, source, class: cls, reviewer: "r", date: "2026-10-23", evidence: "probe" });
const sweepRows = [
  { app: "a", project: "p.json", root: "/c/a", holdout: false, findings: [
    finding("SC1001", "violation", "/c/a/src/x.tsx"), finding("SC1001", "violation", "/c/a/src/y.tsx"),
    finding("SC2001", "uncertifiable", "/c/a/src/z.tsx"), finding("SC8001", "violation", "/c/a/src/s.tsx"),
  ] },
  { app: "b", project: "p.json", root: "/c/b", holdout: true, findings: [
    finding("SC1001", "violation", "/c/b/x.tsx"), finding("SC1001", "violation", "/c/b/y.tsx"),
    finding("SC9001", "violation", "/c/b/n.tsx", 0, 0, { subjectKind: "project" }),
  ] },
];

test("the manifest's holdout flags follow the stated rule: every third distinct app, alphabetically", () => {
  const apps = [...new Set(manifest.projects.map((row) => row.app))].sort();
  const expected = new Set(apps.filter((_, position) => position % 3 === 2));
  assert.ok(manifest.holdoutRule.length > 0);
  for (const row of manifest.projects) assert.equal(row.holdout, expected.has(row.app), `${row.app} ${row.project}`);
  assert.equal(manifest.projects.filter((row) => row.holdout).length, 8);
});

test("selectRows keeps the rows of one split; all keeps every row", () => {
  const rows = [{ app: "a", project: "1", holdout: false }, { app: "b", project: "2", holdout: true }, { app: "c", project: "3", holdout: false }];
  assert.deepEqual(selectRows({ projects: rows }, "dev").map((row) => row.app), ["a", "c"]);
  assert.deepEqual(selectRows({ projects: rows }, "holdout").map((row) => row.app), ["b"]);
  assert.equal(selectRows({ projects: rows }, "all").length, 3);
  const real = (split) => selectRows(manifest, split).length;
  assert.equal(real("dev") + real("holdout"), real("all"));
  assert.equal(real("all"), manifest.projects.length);
});

test("selectRows refuses an unknown split and a row with no boolean holdout", () => {
  assert.throws(() => selectRows({ projects: [] }, "test"), /--split must be one of/);
  assert.throws(() => selectRows({ projects: [{ app: "a", project: "p" }] }, "all"), /no boolean holdout/);
});

test("the sweep's dry run prints the selected rows and a bad split is refused", () => {
  const run = (...args) => spawnSync(process.execPath, [join(here, "corpus-sweep.mjs"), "--dry-run", ...args], { encoding: "utf8" });
  const holdout = run("--split", "holdout");
  assert.equal(holdout.status, 0, holdout.stderr);
  assert.match(holdout.stdout, /8 of 35 rows selected by --split holdout/);
  assert.match(run("--split", "dev").stdout, /27 of 35 rows selected/);
  assert.notEqual(run("--split", "bogus").status, 0);
});

test("splitOf reads the row's flag, else the manifest, else is unknown", () => {
  const index = holdoutIndex({ projects: [{ app: "a", project: "p", holdout: true }] });
  assert.equal(splitOf({ app: "a", project: "p", holdout: false }, index), "dev");
  assert.equal(splitOf({ app: "a", project: "p" }, index), "holdout");
  assert.equal(splitOf({ app: "z", project: "p" }, index), null);
});

test("shareBySplit counts each bucket per split and in total", () => {
  const shares = shareBySplit(sweepRows);
  assert.deepEqual(shares.dev, { violation: 2, uncertifiable: 1, style: 1, notice: 0 });
  assert.deepEqual(shares.holdout, { violation: 2, uncertifiable: 0, style: 0, notice: 1 });
  assert.equal(shares.total.violation, 4);
  assert.equal(bucketOf(finding("SC8001", "violation", "/x")), "style");
});

test("a mixed report with no row flags is split by the manifest", () => {
  const index = holdoutIndex({ projects: [{ app: "a", project: "p.json", holdout: false }, { app: "b", project: "p.json", holdout: true }] });
  const bare = sweepRows.map(({ holdout: _, ...row }) => row);
  assert.deepEqual(shareBySplit(bare, index), shareBySplit(sweepRows));
});

test("the ledger key is the compare key with a path relative to the project root", () => {
  assert.equal(findingKey(sweepRows[0], sweepRows[0].findings[0]), "a|p.json|SC1001|violation|src/x.tsx|1|2");
});

test("an empty ledger is well formed; malformed entries and unknown classes are errors", () => {
  assert.deepEqual(validateLedger({ entries: [] }), []);
  assert.deepEqual(validateLedger({ entries: [entry("a|p|SC1001|violation|x.tsx|1|2", "tsc-owned")] }), []);
  assert.match(validateLedger({ entries: [entry("a|p|SC1001|violation|x.tsx|1|2", "plausible")] }).join(), /class "plausible"/);
  assert.match(validateLedger({ entries: [entry("a|p|SC1001|x.tsx|1|2")] }).join(), /key must be/);
  assert.match(validateLedger({ entries: [{ ...entry("a|p|SC1001|violation|x.tsx|1|2"), date: "yesterday" }] }).join(), /date/);
  assert.match(validateLedger({ entries: [{ ...entry("a|p|SC1001|violation|x.tsx|1|2"), evidence: "" }] }).join(), /evidence/);
  assert.match(validateLedger({ entries: [{ ...entry("a|p|SC1001|violation|x.tsx|1|2"), reviewer: " " }] }).join(), /reviewer/);
  const twice = entry("a|p|SC1001|violation|x.tsx|1|2");
  assert.match(validateLedger({ entries: [twice, twice] }).join(), /listed twice/);
  assert.match(validateLedger({ entries: {} }).join(), /entries must be a list/);
  assert.match(validateLedger({ entries: [{ ...twice, extra: 1 }] }).join(), /unknown fields extra/);
});

test("every entry names its source, and the key has that source's shape", () => {
  const mutant = "a|p.json|src/x.tsx|120|P2";
  assert.deepEqual(validateLedger({ entries: [entry(mutant, "confirmed-violation", "mutants")] }), []);
  const { source: _, ...bare } = entry("a|p|SC1001|violation|x.tsx|1|2");
  assert.match(validateLedger({ entries: [bare] }).join(), /source undefined is not one of corpus-sweep, mutants/);
  assert.match(validateLedger({ entries: [entry(mutant, "confirmed-violation", "seeded")] }).join(), /source "seeded"/);
  assert.match(validateLedger({ entries: [entry(mutant)] }).join(), /key must be app\|project\|id\|kind/);
  assert.match(validateLedger({ entries: [entry("a|p|SC1001|violation|x.tsx|1|2", "confirmed-violation", "mutants")] }).join(), /key must be app\|project\|file\|pos\|pattern/);
  assert.match(validateLedger({ entries: [entry("a|p.json|src/x.tsx|pos|P2", "confirmed-violation", "mutants")] }).join(), /key must be/);
  assert.match(validateLedger({ entries: [entry("a|p.json|src/x.tsx|120|p2", "confirmed-violation", "mutants")] }).join(), /key must be/);
  const twice = entry(mutant, "confirmed-violation", "mutants");
  assert.match(validateLedger({ entries: [twice, twice] }).join(), /listed twice/);
  assert.equal(mutantSiteKey({ app: "a", project: "p.json", file: "src/x.tsx", pos: 120, pattern: "P2", line: 9 }), mutant);
});

test("the committed ledger is well formed, and every mutants key has the seeded-site shape", () => {
  const ledger = JSON.parse(readFileSync(join(here, "corpus/reviewed-findings.json"), "utf8"));
  assert.deepEqual(validateLedger(ledger), []);
  for (const item of ledger.entries.filter((row) => row.source === "mutants")) assert.equal(item.key.split("|").length, 5, item.key);
});

test("reviewed precision counts confirmed over adjudicated; unreviewed and unresolved never enter it", () => {
  const ledger = { entries: [
    entry("a|p.json|SC1001|violation|src/x.tsx|1|2", "adopted-policy-violation"),
    entry("a|p.json|SC1001|violation|src/y.tsx|1|2", "tsc-owned"),
    entry("b|p.json|SC1001|violation|x.tsx|1|2", "unresolved-review"),
  ] };
  const result = reviewedPrecision(sweepRows, ledger);
  assert.equal(result.total.confirmed, 1);
  assert.equal(result.total.adjudicated, 2);
  assert.equal(result.total.precision, 0.5);
  assert.equal(result.total.unresolved, 1);
  assert.equal(result.total.unreviewed, 1, "only b/y.tsx: the SC8001 style finding and the project notice are not violations to review");
  assert.deepEqual(result.total.byClass, { "adopted-policy-violation": 1, "tsc-owned": 1 });
  assert.equal(result.dev.precision, 0.5);
  assert.equal(result.holdout.precision, null, "nothing in holdout is adjudicated");
  assert.equal(result.holdout.unreviewed, 1);
});

test("an empty ledger gives no precision, and every violation is unreviewed", () => {
  const result = reviewedPrecision(sweepRows, { entries: [] });
  assert.equal(result.total.precision, null);
  assert.equal(result.total.unreviewed, 4);
});

test("a ledger entry whose key left a project that ran is stale; one for a project not run is not", () => {
  const ledger = { entries: [
    entry("a|p.json|SC1001|violation|src/gone.tsx|1|2", "valid-for-claim"),
    entry("a|p.json|SC1001|violation|src/also-gone.tsx|1|2"),
    entry("other|p.json|SC1001|violation|src/x.tsx|1|2"),
    entry("a|p.json|SC2001|uncertifiable|src/z.tsx|1|2"),
  ] };
  const result = reviewedPrecision(sweepRows, ledger);
  assert.deepEqual(result.stale, ["a|p.json|SC1001|violation|src/gone.tsx|1|2"], "a gone confirmed violation is an absent miss instead");
  // z.tsx: a confirmed defect recorded against its uncertifiable finding is a withheld miss too.
  assert.deepEqual(confirmedMisses(sweepRows, ledger, null).misses.map((miss) => `${miss.key} ${miss.status}`), [
    "a|p.json|SC1001|violation|src/also-gone.tsx|1|2 absent", "a|p.json|SC2001|uncertifiable|src/z.tsx|1|2 withheld",
  ]);
  assert.equal(result.otherKind, 1);
  assert.equal(result.total.adjudicated, 0);
});

test("a confirmed violation the sweep has only as uncertifiable, or not at all, is a miss per rule", () => {
  const rows = [{ app: "a", project: "p.json", root: "/c/a", findings: [
    finding("SC1001", "uncertifiable", "/c/a/src/w.tsx"), finding("SC4001", "violation", "/c/a/src/v.tsx"),
    finding("SC1001", "uncertifiable", "/c/a/src/u.tsx"),
  ] }];
  const ledger = { entries: [
    entry("a|p.json|SC1001|violation|src/w.tsx|1|2"),
    entry("a|p.json|SC4001|violation|src/v.tsx|1|2"),
    entry("a|p.json|SC4001|violation|src/gone.tsx|1|2"),
    entry("a|p.json|SC1001|violation|src/u.tsx|1|2", "unresolved-review"),
    entry("a|p.json|SC1001|violation|src/t.tsx|1|2", "valid-for-claim"),
    entry("other|p.json|SC1001|violation|src/w.tsx|1|2"),
  ] };
  const result = confirmedMisses(rows, ledger, null);
  assert.deepEqual(result.byRule, {
    SC1001: { withheld: 1, absent: 0, unresolved: 1 },
    SC4001: { withheld: 0, absent: 1, unresolved: 0 },
  });
  assert.deepEqual(result.total, { withheld: 1, absent: 1, unresolved: 1 });
  assert.deepEqual(result.misses.map((miss) => `${miss.rule} ${miss.status}`), ["SC1001 withheld", "SC4001 absent"]);
  const precision = reviewedPrecision(rows, ledger);
  assert.deepEqual(precision.stale, ["a|p.json|SC1001|violation|src/t.tsx|1|2"],
    "a withheld or absent confirmed entry is a miss, not stale; a non-confirmed entry whose finding is gone is stale");
});

test("mutants entries are read against the seeded report only, and never enter reviewed precision", () => {
  const site = (pos, classification, extra = {}) => ({ app: "a", project: "p.json", file: "src/x.tsx", pos, pattern: "P2", rule: "missing-owner",
    classification, findings: [{ id: "SC4001", rule: "missing-owner", kind: classification === "proven" ? "violation" : "uncertifiable" }], ...extra });
  const report = { sites: [site(1, "uncertifiable"), site(2, "proven"), site(3, "missed", { findings: [] }), site(4, "uncertifiable")] };
  const ledger = { entries: [
    entry("a|p.json|src/x.tsx|1|P2", "confirmed-violation", "mutants"),
    entry("a|p.json|src/x.tsx|2|P2", "confirmed-violation", "mutants"),
    entry("a|p.json|src/x.tsx|3|P2", "confirmed-violation", "mutants"),
    entry("a|p.json|src/x.tsx|4|P2", "unresolved-review", "mutants"),
    entry("a|p.json|src/x.tsx|9|P2", "confirmed-violation", "mutants"),
  ] };
  const result = confirmedMisses(sweepRows, ledger, report);
  assert.deepEqual(result.byRule, { SC4001: { withheld: 1, absent: 0, unresolved: 1 }, "missing-owner": { withheld: 0, absent: 1, unresolved: 0 } });
  assert.equal(confirmedMisses(sweepRows, ledger, null).misses.length, 0, "no report: no mutants entry is in scope");
  const precision = reviewedPrecision(sweepRows, ledger);
  assert.equal(precision.total.adjudicated, 0);
  assert.deepEqual(precision.stale, []);
});

test("corpus-metrics --check-ledger passes on the committed ledger and fails on a malformed one", () => {
  const script = join(here, "corpus-metrics.mjs");
  const ok = spawnSync(process.execPath, [script, "--check-ledger"], { encoding: "utf8" });
  assert.equal(ok.status, 0, ok.stderr);
  const dir = mkdtempSync(join(tmpdir(), "ledger-"));
  const bad = join(dir, "bad.json");
  writeFileSync(bad, JSON.stringify({ entries: [entry("a|p|SC1001|violation|x.tsx|1|2", "maybe")] }));
  const failed = spawnSync(process.execPath, [script, "--check-ledger", "--ledger", bad], { encoding: "utf8" });
  assert.equal(failed.status, 1);
  assert.match(failed.stderr, /class "maybe"/);
});

test("corpus-metrics prints per-split and reviewed numbers, and compare ignores the new report fields", () => {
  const dir = mkdtempSync(join(tmpdir(), "metrics-"));
  const sweep = join(dir, "sweep.json");
  writeFileSync(sweep, JSON.stringify({ split: "all", results: sweepRows }));
  const out = execFileSync(process.execPath, [join(here, "corpus-metrics.mjs"), sweep], { encoding: "utf8" });
  assert.match(out, /by split \(sweep ran: all\)/);
  assert.match(out, /reviewed precision/);
  assert.match(out, /unreviewed 4/);
  assert.match(out, /confirmed misses \(confirmed-violation entries not proven now; mutants entries not read: no --mutants\)/);
  const compare = execFileSync(process.execPath, [join(here, "corpus-sweep-compare.mjs"), sweep, sweep], { encoding: "utf8" });
  assert.match(compare, /removed 0/);
});
