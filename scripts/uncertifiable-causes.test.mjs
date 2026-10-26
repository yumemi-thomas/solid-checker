import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { test } from "vitest";

import { afterSuspension } from "./uncertifiable-causes.mjs";

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
