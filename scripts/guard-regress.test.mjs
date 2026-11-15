import assert from "node:assert/strict";
import { test } from "vitest";

import { baseCandidates, baseFile } from "./guard/base-revision.mjs";
import { checkRegress, cleanToU, measureRegress } from "./guard/regress.mjs";

// ADR 0995: a proved violation must not silently become unproven, and no base is a failure.
const v = (code, path, start, end) => ({ kind: "violation", code, path, start, end });
const u = (code, path, start, end, blocker = "callee-callback-timing") => ({ kind: "uncertifiable", code, path, start, end, blocker });
const snap = (findings, status = "violation") => ({ status, findings });
const decisions = '<a id="adr-0995"></a>\n## ADR 0995 — x\n';
const unchanged = () => false;
const changed = () => true;

test("a violation that became uncertifiable at its span is a removal, though the count of findings holds", () => {
  // The laundering: the proved violation at 10-20 is now an uncertifiable row at the same span: findings 1 -> 1, uncertifiable rows +1,
  // which count.mjs holds one way and rises.mjs prints and never fails. Nothing recorded that a proved result stopped being one.
  const base = new Map([["reactive-ir/a", snap([v("SC1001", "a/App.tsx", 10, 20)])]]);
  const now = new Map([["reactive-ir/a", snap([u("SC1001", "a/App.tsx", 10, 20)], "uncertifiable")]]);
  const measured = measureRegress(base, now, unchanged);
  assert.deepEqual(measured, [{ fixture: "reactive-ir/a", path: "a/App.tsx", code: "SC1001", removed: 1, uncertifiable: 1, gone: 0, blockers: ["callee-callback-timing"], mode: "span" }]);
  const { failures } = checkRegress({ ledger: { regress: [] }, base: { regress: [] }, measured, decisions });
  assert.equal(failures.length, 1);
  assert.match(failures[0], /1 violation\(s\) left \(1 became uncertifiable \(callee-callback-timing\), 0 gone; by span\) with no regress row/);
});

test("a violation that is gone altogether is a removal too", () => {
  const base = new Map([["reactive-ir/a", snap([v("SC1001", "a/App.tsx", 10, 20), v("SC1001", "a/App.tsx", 30, 40)])]]);
  const now = new Map([["reactive-ir/a", snap([v("SC1001", "a/App.tsx", 30, 40)])]]);
  const [item] = measureRegress(base, now, unchanged);
  assert.deepEqual({ removed: item.removed, uncertifiable: item.uncertifiable, gone: item.gone }, { removed: 1, uncertifiable: 0, gone: 1 });
});

test("a violation that stays at its span is no removal; a code or a path that differs is", () => {
  const base = new Map([["reactive-ir/a", snap([v("SC1001", "a/App.tsx", 10, 20)])]]);
  assert.deepEqual(measureRegress(base, new Map([["reactive-ir/a", snap([v("SC1001", "a/App.tsx", 10, 20), u("SC1001", "a/App.tsx", 50, 60)])]]), unchanged), []);
  assert.equal(measureRegress(base, new Map([["reactive-ir/a", snap([v("SC2001", "a/App.tsx", 10, 20)])]]), unchanged).length, 1);
  assert.equal(measureRegress(base, new Map([["reactive-ir/a", snap([v("SC1001", "a/Other.tsx", 10, 20)])]]), unchanged).length, 1);
});

test("a changed file is compared by count, not by span: a violation swapped for another is hidden only by counting", () => {
  // The file's bytes changed, so spans moved: one violation at 10-20 and one new at 100-110 are 1 -> 1 and nothing left.
  const base = new Map([["reactive-ir/a", snap([v("SC1001", "a/App.tsx", 10, 20)])]]);
  assert.deepEqual(measureRegress(base, new Map([["reactive-ir/a", snap([v("SC1001", "a/App.tsx", 100, 110)])]]), changed), []);
  // Two became one: a removal, by count.
  const two = new Map([["reactive-ir/a", snap([v("SC1001", "a/App.tsx", 10, 20), v("SC1001", "a/App.tsx", 30, 40)])]]);
  const [item] = measureRegress(two, new Map([["reactive-ir/a", snap([v("SC1001", "a/App.tsx", 100, 110), u("SC1001", "a/App.tsx", 200, 210, "x-blocker")])]]), changed);
  assert.deepEqual({ removed: item.removed, uncertifiable: item.uncertifiable, gone: item.gone, blockers: item.blockers, mode: item.mode }, { removed: 1, uncertifiable: 1, gone: 0, blockers: ["x-blocker"], mode: "count" });
});

test("a deleted fixture's violations are gone", () => {
  const base = new Map([["reactive-ir/a", snap([v("SC1001", "a/App.tsx", 10, 20)])]]);
  const [item] = measureRegress(base, new Map(), unchanged);
  assert.equal(item.gone, 1);
});

test("a row names a removal with an ADR in docs/decisions.md and says why", () => {
  const measured = [{ fixture: "reactive-ir/a", path: "a/App.tsx", code: "SC1001", removed: 1, uncertifiable: 1, gone: 0, blockers: [], mode: "span" }];
  const row = { fixture: "reactive-ir/a", path: "a/App.tsx", code: "SC1001", removed: 1, adr: "ADR 0995", why: "the callback may run elsewhere: withheld" };
  assert.deepEqual(checkRegress({ ledger: { regress: [row] }, base: { regress: [] }, measured, decisions }).failures, []);
  for (const broken of [{ adr: "ADR 0999" }, { adr: undefined }, { why: "" }, { removed: 2 }, { path: "a/Other.tsx" }, { code: "SC2001" }]) {
    const { failures } = checkRegress({ ledger: { regress: [{ ...row, ...broken }] }, base: { regress: [] }, measured, decisions });
    assert.ok(failures.length >= 1, JSON.stringify(broken));
  }
  // Append-only: a row at the base cannot be dropped to rename it.
  assert.match(checkRegress({ ledger: { regress: [] }, base: { regress: [row] }, measured: [], decisions }).failures[0], /append-only/);
});

test("a certified fixture that gained uncertifiable rows is reported, never failed", () => {
  const base = new Map([["reactive-ir/a", snap([], "certified")], ["reactive-ir/b", snap([v("SC1001", "b.tsx", 1, 2)])]]);
  const now = new Map([["reactive-ir/a", snap([u("SC1001", "a.tsx", 1, 2)], "uncertifiable")], ["reactive-ir/b", snap([u("SC1001", "b.tsx", 1, 2)], "uncertifiable")]]);
  assert.deepEqual(cleanToU(base, now), [{ fixture: "reactive-ir/a", rows: 1 }]);
});

test("no base is a failure, never a pass; COUNT_BASE names one and a ref that does not resolve is a failure too", () => {
  const nothing = () => null;
  assert.match(baseCandidates({}, nothing).problem, /no base revision.*COUNT_BASE=<rev>.*fetch-depth: 0/);
  assert.match(baseCandidates({ COUNT_BASE: "nope" }, nothing).problem, /COUNT_BASE=nope names no commit/);
  const resolves = (...args) => (args[0] === "rev-parse" ? "abc123" : null);
  assert.deepEqual(baseCandidates({ COUNT_BASE: "main" }, resolves), { revisions: ["abc123"], problem: null });
  // A merge base that resolves is the base; a base without the file is a failure, not a skipped comparison.
  const merged = (command) => (command === "merge-base" ? "def456" : null);
  assert.deepEqual(baseCandidates({}, merged).revisions, ["def456", "def456"]);
  assert.match(baseFile("scripts/guard/counts.json", {}, merged).problem, /does not exist at the base/);
  const shown = (command) => (command === "merge-base" ? "def456" : command === "show" ? "{}" : null);
  assert.equal(baseFile("scripts/guard/counts.json", {}, shown).text, "{}");
});
