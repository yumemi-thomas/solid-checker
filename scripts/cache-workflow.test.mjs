import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "vitest";

const ci = readFileSync(".github/workflows/ci.yml", "utf8");
const publish = readFileSync(".github/workflows/publish-npm.yml", "utf8");
const contractCorpus = readFileSync(".github/workflows/contract-corpus.yml", "utf8");
const ecosystemBenchmark = readFileSync(".github/workflows/ecosystem-benchmark.yml", "utf8");

function jobBody(workflow, name) {
  const lines = workflow.split("\n");
  const start = lines.findIndex((line) => line === `  ${name}:`);
  assert.notEqual(start, -1, `missing ${name} job`);

  let end = lines.length;
  for (let index = start + 1; index < lines.length; index += 1) {
    if (/^  [a-zA-Z0-9_-]+:$/.test(lines[index])) {
      end = index;
      break;
    }
  }
  return lines.slice(start, end).join("\n");
}

function matrixTargets(body) {
  const lines = body.split("\n");
  const targets = [];
  for (let index = 0; index < lines.length; index += 1) {
    const runner = lines[index].match(/^          - runner: (.+)$/)?.[1];
    if (!runner) continue;

    const platform = lines[index + 1]?.match(/^            platform: (.+)$/)?.[1];
    const arch = lines[index + 2]?.match(/^            arch: (.+)$/)?.[1];
    assert.ok(platform && arch, `incomplete matrix target for ${runner}`);
    targets.push({ runner, platform, arch });
  }
  return targets;
}

function sharedKey(body) {
  return body.match(/^\s+shared-key:\s*(.+)$/m)?.[1].trim();
}

function actionInput(body, name) {
  const escaped = name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return body.match(new RegExp(`^\\s+${escaped}:\\s*(.+)$`, "m"))?.[1].trim();
}

test("main warms every native Rust cache consumed while publishing", () => {
  const mainTargets = new Set(
    matrixTargets(jobBody(ci, "rust-package")).map(JSON.stringify),
  );

  for (const target of matrixTargets(jobBody(publish, "build"))) {
    assert.ok(
      mainTargets.has(JSON.stringify(target)),
      `main does not warm the publishing cache for ${JSON.stringify(target)}`,
    );
  }
});

test("main and publishing share one explicit WASM Rust cache key", () => {
  const mainKey = sharedKey(jobBody(ci, "wasm-package"));
  const publishKey = sharedKey(jobBody(publish, "wasm"));

  assert.ok(mainKey, "main's WASM Rust cache key must be explicit");
  assert.equal(publishKey, mainKey);
});

test("native caches do not restore macOS target executables", () => {
  const main = jobBody(ci, "rust-package");
  const release = jobBody(publish, "build");
  const safeTargetPolicy = "${{ matrix.platform != 'darwin' }}";

  assert.equal(sharedKey(release), sharedKey(main));
  assert.equal(actionInput(main, "cache-targets"), safeTargetPolicy);
  assert.equal(actionInput(release, "cache-targets"), safeTargetPolicy);
});

test("the contract corpus installs and watches its temporary-v2 producer", () => {
  const install = contractCorpus.indexOf(
    "bun install --cwd packages/cli --ignore-scripts --no-progress --frozen-lockfile"
  );
  const run = contractCorpus.indexOf("run: make contract-corpus");

  assert.ok(install >= 0, "contract-corpus must install the producer's runtime dependencies");
  assert.ok(install < run, "producer dependencies must be installed before contract generation");
  assert.match(contractCorpus, /- "packages\/cli\/scripts\/\*\*"/);
  assert.doesNotMatch(contractCorpus, /generate-package-contract\.mjs/);
});

test("the PR ecosystem sentinel keeps its real timeout probe within the CI runner envelope", () => {
  const sentinel = jobBody(ecosystemBenchmark, "sentinel");

  assert.match(
    sentinel,
    /^\s+run: bun scripts\/ecosystem-benchmark\/run\.mjs --sentinel --timeout 120 --concurrency 8$/m,
  );
});
