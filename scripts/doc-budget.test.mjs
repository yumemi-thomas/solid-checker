import assert from "node:assert/strict";
import { test } from "vitest";

import { FILE_BUDGETS, FIXTURE_README_MAX, measuredSizes, violations } from "./doc-budget.mjs";

test("the repository is within its documentation budget", () => {
  assert.deepEqual(violations(measuredSizes()), []);
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
