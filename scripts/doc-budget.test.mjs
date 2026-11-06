import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { join } from "node:path";
import { test } from "vitest";

import { FILE_BUDGETS, FIXTURE_README_MAX, ROOT, measuredSizes, report, violations } from "./doc-budget.mjs";

// Suspended during the flip (scripts/doc-budget.mjs): this stood as `violations(measuredSizes())` equals `[]`, and returns before #173 merges.
test("a run reports the measured sizes and everything over a limit, and does not fail", () => {
  const sizes = measuredSizes();
  const lines = report(sizes).join("\n");
  for (const path of Object.keys(FILE_BUDGETS)) assert.match(lines, new RegExp(`${path.replaceAll(".", "\\.")}: ${sizes[path] ?? 0} of ${FILE_BUDGETS[path]} bytes`));
  for (const found of violations(sizes)) assert.ok(lines.includes(found), found);
  const run = spawnSync("bun", [join(ROOT, "scripts/doc-budget.mjs")], { encoding: "utf8" });
  assert.equal(run.status, 0, run.stderr);
  assert.match(run.stdout, /doc-budget \(suspended\)/);
});

test("the report names what is over, and says when nothing is", () => {
  assert.match(report({ "AGENTS.md": FILE_BUDGETS["AGENTS.md"] + 1 }).join("\n"), /1 over, suspended[^\n]*\n {2}AGENTS\.md is/);
  assert.ok(report({}).includes("nothing over"));
});

test("an oversized instruction file is reported", () => {
  const found = violations({ "AGENTS.md": FILE_BUDGETS["AGENTS.md"] + 1 });
  assert.equal(found.length, 1);
  assert.match(found[0], /AGENTS\.md/);
});

test("a new top-level document in docs/ is reported, an allowed one is not", () => {
  assert.equal(violations({ "docs/2026-10-20-plan.md": 10 }).length, 1);
  assert.deepEqual(violations({ "docs/rules/new-rule.md": 10, "docs/decisions.md": 10 }), []);
});

test("a long fixture README is reported; fixture sources are not measured", () => {
  assert.equal(violations({ "fixtures/reactive-ir/x/README.md": FIXTURE_README_MAX + 1 }).length, 1);
  assert.deepEqual(violations({ "fixtures/reactive-ir/x/App.tsx": 10_000_000 }), []);
});
