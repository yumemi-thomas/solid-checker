import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { test } from "vitest";

const root = resolve(import.meta.dirname, "..");
const example = resolve(root, "examples/solid-2-dev-app");

// The example app is the packaged CLI's reference consumer: it must depend on
// the workspace CLI package and lint through the packaged entry points, not
// through leftover local scripts.
test("the example app consumes the packaged CLI", () => {
  const packageJson = JSON.parse(
    readFileSync(resolve(example, "package.json"), "utf8")
  );
  assert.equal(
    packageJson.devDependencies?.["solid-checker"],
    "file:../../packages/cli"
  );
  assert.equal(packageJson.scripts?.lint, "oxlint");
  assert.equal(packageJson.scripts?.["lint:fix"], "oxlint --fix");
  assert.equal(existsSync(resolve(example, "scripts/lint.mjs")), false);
});
