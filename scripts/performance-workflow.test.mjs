import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "vitest";

const workflow = readFileSync(".github/workflows/performance.yml", "utf8");
const codspeed = readFileSync(".github/workflows/codspeed.yml", "utf8");
const configuration = readFileSync("codspeed.yml", "utf8");
const runner = readFileSync("benchmarks/run-codspeed-case.mjs", "utf8");

test("performance regressions are gated on pull requests and the default branch", () => {
  assert.match(workflow, /\bpush:\s*\n\s+branches:\s*\n\s+- main\b/);
  assert.match(workflow, /\bpull_request:/);
  assert.match(workflow, /bun benchmarks\/verify-performance\.mjs/);
  assert.match(
    workflow,
    /SOLID_CHECKER_MAX_FIRST_IR_NS_PER_SOURCE: 225000/,
  );
  assert.match(workflow, /git merge-base HEAD/);
  assert.match(workflow, /git rev-parse HEAD\^/);
  assert.match(workflow, /bun benchmarks\/compare-performance\.mjs/);
});

test("CodSpeed measures the same commits, in its own workflow", () => {
  assert.match(codspeed, /\bpush:\s*\n\s+branches:\s*\n\s+- main\b/);
  assert.match(codspeed, /\bpull_request:/);
  assert.match(codspeed, /CodSpeedHQ\/action@v5/);
  // Wall time because the end-to-end analysis includes the Type Facts child
  // process, which CPU simulation does not follow.
  assert.match(codspeed, /\bmode: walltime\b/);
  // The action authenticates with an OIDC token minted for the run, so the
  // repository holds no upload secret.
  assert.match(codspeed, /\bid-token: write\b/);
  assert.match(codspeed, /bun benchmarks\/generate-bench-corpus\.mjs/);
  // The corpus the cases read is the one this workflow generates.
  assert.match(
    codspeed,
    /SOLID_CHECKER_CODSPEED_CORPUS: \/tmp\/solid-checker-codspeed-corpus/,
  );
  // A measurement is not a gate: keep it out of the workflow whose red gates
  // would stop the commit from being measured at all.
  assert.doesNotMatch(workflow, /CodSpeedHQ\/action/);
});

test("every measured path has an independent history and a case to run", () => {
  for (const id of [
    "fresh-analysis-1000",
    "cached-analysis-1000",
    "incremental-analysis-1000",
    "structural-analysis-1000",
  ]) {
    assert.match(configuration, new RegExp(`\\bid: ${id}\\b`));
  }

  for (const scenario of ["fresh", "cached", "incremental", "structural"]) {
    assert.match(
      configuration,
      new RegExp(`run-codspeed-case\\.mjs ${scenario}\\b`),
    );
    assert.match(runner, new RegExp(`^  ${scenario}: \\[`, "m"));
  }
});
