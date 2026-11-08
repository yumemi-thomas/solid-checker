import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "vitest";

import {
  FILE_BUDGETS,
  FIXTURE_README_MAX,
  RAISES,
  ROOT,
  baseSource,
  currentLimits,
  limitsOf,
  measuredSizes,
  raiseRowsOf,
  ratchetViolations,
  violations,
} from "./doc-budget.mjs";

test("the repository is within its documentation budget", () => {
  assert.deepEqual(violations(measuredSizes()), []);
});

// The ratchet (owner ruling, 2026-11-08): a limit rises only with a RAISES row naming the ruling.
test("no limit rose against the base without its owner ruling", () => {
  const base = baseSource();
  if (!base) return;
  const source = readFileSync(join(ROOT, "scripts/doc-budget.mjs"), "utf8");
  assert.deepEqual(ratchetViolations(currentLimits(), limitsOf(base.text), RAISES, raiseRowsOf(base.text), raiseRowsOf(source)), []);
});

test("the limits read from the source are the limits in force", () => {
  assert.deepEqual(limitsOf(readFileSync(join(ROOT, "scripts/doc-budget.mjs"), "utf8")), currentLimits());
});

test("a rise needs its exact row; a fall needs none; base rows stay", () => {
  const base = { "docs/decisions.md": 100, "docs/": 200 };
  assert.equal(ratchetViolations({ "docs/decisions.md": 101, "docs/": 200 }, base, []).length, 1);
  assert.equal(ratchetViolations({ "docs/decisions.md": 101 }, base, [{ limit: "docs/decisions.md", from: 100, to: 102, ruling: "x" }]).length, 1);
  assert.equal(ratchetViolations({ "docs/decisions.md": 101 }, base, [{ limit: "docs/decisions.md", from: 100, to: 101 }]).length, 1);
  assert.deepEqual(ratchetViolations({ "docs/decisions.md": 101 }, base, [{ limit: "docs/decisions.md", from: 100, to: 101, ruling: "owner" }]), []);
  assert.deepEqual(ratchetViolations({ "docs/decisions.md": 90, "docs/": 150 }, base, []), []);
  assert.equal(ratchetViolations({}, base, [], ["{ a }"], []).length, 1);
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
