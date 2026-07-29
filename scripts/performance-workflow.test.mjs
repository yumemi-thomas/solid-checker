import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const workflow = readFileSync(".github/workflows/performance.yml", "utf8");
const configuration = readFileSync("codspeed.yml", "utf8");

test("performance regressions are compared on pull requests and the default branch", () => {
  assert.match(workflow, /\bpush:\s*\n\s+branches:\s*\n\s+- main\b/);
  assert.match(workflow, /\bpull_request:/);
  assert.match(workflow, /CodSpeedHQ\/action@v4/);
  assert.match(workflow, /\bmode: walltime\b/);
  assert.match(workflow, /node benchmarks\/verify-performance\.mjs/);
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
