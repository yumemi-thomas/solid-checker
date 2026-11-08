import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { test } from "vitest";

import { afterSuspension, heldProblems } from "./uncertifiable-causes.mjs";

const ts = createRequire(new URL("../packages/cli/package.json", import.meta.url))("typescript");

/** `afterSuspension` for the `site()` call in the one async function of `source`. */
function siteAfterSuspension(source) {
  const file = ts.createSourceFile("a.ts", source, ts.ScriptTarget.Latest, true);
  let fn;
  let site;
  const visit = (node) => {
    if (!fn && (ts.isFunctionDeclaration(node) || ts.isArrowFunction(node))) fn = node;
    if (ts.isCallExpression(node) && node.expression.getText(file) === "site") site = node;
    ts.forEachChild(node, visit);
  };
  visit(file);
  return afterSuspension(fn, site);
}

test("a site follows an await written before it", () => {
  assert.equal(siteAfterSuspension("async function f() { site(); await g(); }"), false);
  assert.equal(siteAfterSuspension("async function f() { await g(); site(); }"), true);
  assert.equal(siteAfterSuspension("function f() { site(); }"), false);
});

test("a loop's back edge puts a site after an await written later in the loop", () => {
  assert.equal(siteAfterSuspension("async function f() { while (c) { site(); await g(); } }"), true);
  assert.equal(siteAfterSuspension("async function f() { for (const x of xs) { site(); await g(x); } }"), true);
  assert.equal(siteAfterSuspension("async function f() { for (;;) { if (c) site(); else await g(); } }"), true);
  assert.equal(siteAfterSuspension("async function f() { for (const x of xs) { site(); } await g(); }"), false);
});

test("what a loop runs once, and an await in a nested function, are no back edge", () => {
  assert.equal(siteAfterSuspension("async function f() { for (let i = site(); c; ) { await g(); } }"), false);
  assert.equal(siteAfterSuspension("async function f() { for (const x of site()) { await g(x); } }"), false);
  assert.equal(siteAfterSuspension("async function f() { while (c) { site(); h(async () => { await g(); }); } }"), false);
});

/** A one-project report whose findings are `rows` ([kind, id, analysisContext]) in a.tsx. */
function heldReport(rows) {
  return {
    results: [
      {
        app: "p",
        root: "/r",
        findings: rows.map(([kind, id, analysisContext]) => ({ kind, id, analysisContext, primaryLocation: { path: "/r/a.tsx" } })),
      },
    ],
  };
}
const OPEN_WORK = "Intro.\n\n**Untyped `.bind` in dependency JS (ADR 0803 step 2):** later proof.\n";
const heldCase = (held, id = "c") => ({
  id,
  kind: "uncertifiable",
  project: "p",
  files: ["a.tsx"],
  causes: ["SC2001/unplaced-write"],
  expect: [{ rule: "SC2001", family: "unplaced-write", context: "", cause: "SC2001/unplaced-write" }],
  held: { adr: "0803", restoredBy: "Untyped `.bind` in dependency JS (ADR 0803 step 2)", why: "w", ...held },
});
const TIW = { kind: "uncertifiable", rule: "SC9012", context: "tuple-iteration-written" };

test("a held case passes when its files produce exactly its held rows", () => {
  const report = heldReport([["uncertifiable", "SC9012", "tuple-iteration-written"]]);
  assert.deepEqual(heldProblems([heldCase({ expect: [TIW] })], report, OPEN_WORK, 1), []);
});

test("a held case fails on a missing or an extra row", () => {
  const missing = heldReport([]);
  assert.equal(heldProblems([heldCase({ expect: [TIW] })], missing, OPEN_WORK, 1).length > 0, true);
  const extra = heldReport([
    ["uncertifiable", "SC9012", "tuple-iteration-written"],
    ["uncertifiable", "SC1001", "read-backing-unproven"],
  ]);
  assert.equal(heldProblems([heldCase({ expect: [TIW] })], extra, OPEN_WORK, 1).length > 0, true);
});

test("a held case that goes clean or shows a violation fails", () => {
  const clean = heldProblems([heldCase({ expect: [] })], heldReport([]), OPEN_WORK, 1);
  assert.ok(clean.some((problem) => problem.includes("must stay uncertifiable")));
  const violation = heldProblems(
    [heldCase({ expect: [{ kind: "violation", rule: "SC2001", context: "" }] })],
    heldReport([["violation", "SC2001", ""]]),
    OPEN_WORK,
    1,
  );
  assert.ok(violation.some((problem) => problem.includes("must stay uncertifiable")));
});

test("a held case needs its ADR and an open-work item that restores it", () => {
  const report = heldReport([["uncertifiable", "SC9012", "tuple-iteration-written"]]);
  assert.ok(heldProblems([heldCase({ expect: [TIW], adr: undefined })], report, OPEN_WORK, 1).some((problem) => problem.includes("no ADR")));
  assert.ok(heldProblems([heldCase({ expect: [TIW], restoredBy: undefined })], report, OPEN_WORK, 1).some((problem) => problem.includes("no restoredBy")));
  assert.ok(heldProblems([heldCase({ expect: [TIW], restoredBy: "Not an item" })], report, OPEN_WORK, 1).some((problem) => problem.includes("exact title")));
});

test("the held count must equal the cap: over or under it fails", () => {
  const report = heldReport([["uncertifiable", "SC9012", "tuple-iteration-written"]]);
  const cases = [heldCase({ expect: [TIW] }, "a"), heldCase({ expect: [TIW] }, "b")];
  assert.deepEqual(heldProblems(cases, report, OPEN_WORK, 2), []);
  assert.ok(heldProblems(cases, report, OPEN_WORK, 1).some((problem) => problem.includes("the cap is 1")));
  assert.ok(heldProblems(cases, report, OPEN_WORK, 3).some((problem) => problem.includes("the cap is 3")));
});

test("restoredBy must equal an open-work title exactly, not a part of one", () => {
  const report = heldReport([["uncertifiable", "SC9012", "tuple-iteration-written"]]);
  const partial = heldCase({ expect: [TIW], restoredBy: "Untyped `.bind`" });
  assert.ok(heldProblems([partial], report, OPEN_WORK, 1).some((problem) => problem.includes("exact title")));
  const prose = heldCase({ expect: [TIW], restoredBy: "later proof." });
  assert.ok(heldProblems([prose], report, OPEN_WORK, 1).some((problem) => problem.includes("exact title")));
});
