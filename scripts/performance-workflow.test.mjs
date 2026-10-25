import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "vitest";

const workflow = readFileSync(".github/workflows/performance.yml", "utf8");
const configuration = readFileSync("codspeed.yml", "utf8");

test("performance regressions are compared on pull requests and the default branch", () => {
  assert.match(workflow, /\bpush:\s*\n\s+branches:\s*\n\s+- main\b/);
  assert.match(workflow, /\bpull_request:/);
  assert.match(workflow, /CodSpeedHQ\/action@v4/);
  assert.match(workflow, /\bmode: walltime\b/);
  // Shared runners report the wall-time ceilings instead of enforcing them;
  // the interleaved base/head comparison is the regression gate there.
  assert.match(workflow, /bun benchmarks\/verify-performance\.mjs --wall-time-gate report/);
  assert.doesNotMatch(workflow, /SOLID_CHECKER_MAX_FIRST_IR_NS_PER_SOURCE/);
  assert.match(workflow, /git merge-base HEAD/);
  assert.match(workflow, /git rev-parse HEAD\^/);
  assert.match(workflow, /bun benchmarks\/compare-performance\.mjs/);
  // Accepted regressions are scoped by the base runtime's producer and IR.
  assert.match(workflow, /--base-producer /);
  assert.match(workflow, /--base-ir /);
  assert.match(workflow, /solid-reactive-ir\/src\/base\/memo\.rs/);
});

test("fresh, cached, and incremental paths have independent histories", () => {
  for (const id of [
    "fresh-analysis-1000",
    "cached-analysis-1000",
    "incremental-analysis-1000",
  ]) {
    assert.match(configuration, new RegExp(`\\bid: ${id}\\b`));
  }
});
