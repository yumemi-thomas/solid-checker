import assert from "node:assert/strict";
import { mkdtempSync, writeFileSync, readFileSync, rmSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "vitest";

const plugin = createRequire(import.meta.url)("../eslint.cjs");

test("runtime resolution is explicit and every opted-in lint observes fresh answers", () => {
  const root = mkdtempSync(join(tmpdir(), "solid-runtime-inference-"));
  try {
    const project = join(root, "tsconfig.json");
    const state = join(root, "state.json");
    const analyzer = join(root, "analyzer.mjs");
    writeFileSync(project, "{}");
    writeFileSync(analyzer, `import { existsSync, readFileSync, writeFileSync } from "node:fs";
const state = process.argv[2];
const previous = existsSync(state) ? JSON.parse(readFileSync(state, "utf8")) : { count: 0 };
writeFileSync(state, JSON.stringify({ count: previous.count + 1, args: process.argv.slice(3), resolver: process.env.SOLID_CHECKER_RUNTIME_RESOLVER }));
process.stdout.write(JSON.stringify({ status: "certified", findings: [] }));`);
    const context = runtimeResolution => ({
      filename: join(root, "main.ts"),
      settings: { solidChecker: {
        project, command: process.execPath, commandArgs: [analyzer, state],
        runtime: { target: "browser" },
        ...(runtimeResolution === undefined ? {} : { runtimeResolution })
      } }
    });
    const observed = () => JSON.parse(readFileSync(state, "utf8"));
    plugin._testing.snapshotCache.clear();
    plugin._testing.loadSnapshot(context());
    plugin._testing.loadSnapshot(context());
    assert.equal(observed().count, 1);
    assert(!observed().args.includes("--runtime-resolution"));
    plugin._testing.loadSnapshot(context("required"));
    plugin._testing.loadSnapshot(context("required"));
    assert.equal(observed().count, 3);
    const position = observed().args.indexOf("--runtime-resolution");
    assert.equal(observed().args[position + 1], "required");
    assert(observed().resolver.endsWith("runtime-resolver.mjs"));
    assert.throws(() => plugin._testing.loadSnapshot(context("automatic")), /runtimeResolution must be required or off/);
    plugin._testing.loadSnapshot(context("off"));
    assert.equal(observed().count, 3, "explicit off shares the default cache key");
  } finally {
    plugin._testing.snapshotCache.clear();
    rmSync(root, { recursive: true, force: true });
  }
});
