import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "vitest";
import { runCoverageUnit } from "./lib/coverage-cache.mjs";
import { hashTree, openGateCache } from "./lib/gate-cache.mjs";

test("warm inference units recompute after absent ancestor PostCSS and package mutations", async () => {
  const root = mkdtempSync(join(tmpdir(), "coverage-inference-"));
  try {
    const original = join(root, "fixtures", "spa");
    const ancestor = join(root, "rust", "target", "fixture-authorization");
    const materialized = join(ancestor, "spa", "project");
    mkdirSync(original, { recursive: true });
    mkdirSync(materialized, { recursive: true });
    const script = join(root, "coverage.mjs");
    writeFileSync(script, "// test gate\n");
    const cache = openGateCache({ gate: "coverage", root, scriptPath: script, binaries: [], env: {} });
    const parts = () => [hashTree(original)];
    let calls = 0;
    const compute = async () => {
      calls += 1;
      const postcss = existsSync(join(ancestor, "postcss.config.cjs"));
      const published = existsSync(join(ancestor, "package.json")) &&
        JSON.parse(readFileSync(join(ancestor, "package.json"), "utf8")).exports !== undefined;
      return { violations: postcss || published ? 0 : 1 };
    };
    // Seed the exact stale entry an old coverage key could replay.
    await cache.run(parts, compute);
    assert.equal((await cache.run(parts, compute)).hit, true);
    for (const [name, bytes] of [["postcss.config.cjs", "throw 0"], ["package.json", '{"exports":"./index.js"}']]) {
      writeFileSync(join(ancestor, name), bytes);
      const warm = await runCoverageUnit(cache, parts, compute, []);
      assert.deepEqual(warm.value, await compute());
      assert.equal(warm.value.violations, 0);
      assert.equal(warm.hit, false);
      rmSync(join(ancestor, name));
      const restored = await runCoverageUnit(cache, parts, compute, []);
      assert.equal(restored.value.violations, 1);
      assert.equal(restored.hit, false);
    }
    assert.equal(calls, 7);
    assert.equal((await runCoverageUnit(cache, parts, compute, ["--runtime-target", "browser"])).hit, true);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
