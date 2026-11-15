import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

// ADR 0995: the ratchets run in CI, with the history their base needs.
const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const ci = readFileSync(join(root, ".github/workflows/ci.yml"), "utf8");
const job = (name) => {
  const start = ci.indexOf(`\n  ${name}:\n`);
  assert.ok(start >= 0, `${name} job`);
  const next = ci.slice(start + 1).search(/\n  [a-z-]+:\n/);
  return ci.slice(start, next < 0 ? undefined : start + 1 + next);
};

test("the ratchets that need no checker build run in a CI job that fetches the whole history", () => {
  // The laundering: nothing in CI ran count, rises, proof-lint or ir-read-lint, so a pull request that moved a ratchet was held to it by
  // whoever ran `make verify`; and with a shallow clone they would have had no merge base to compare with.
  const guards = job("guards");
  assert.match(guards, /fetch-depth: 0/);
  for (const command of ["node scripts/guard/count.mjs", "node scripts/guard/rises.mjs", "node scripts/guard/regress.mjs", "node scripts/guard/base.mjs", "bun scripts/proof-lint.mjs", "bun scripts/ir-read-lint.mjs"]) {
    assert.ok(guards.includes(`- run: ${command}`), command);
  }
  // Nothing in it builds the checker.
  assert.ok(!/cargo|make /.test(guards.replace(/#[^\n]*/g, "")), "the guards job builds nothing");
  assert.match(ci, /needs: \[[^\]]*\bguards\b[^\]]*\]/, "the tested-tree record waits for it");
});

test("the engine job runs the proof gate and the reproduction causes after the coverage gate", () => {
  const engine = job("rust-engine");
  const at = (step) => engine.indexOf(`- run: ${step}`);
  assert.ok(at("make coverage") >= 0 && at("make proof-gate") > at("make coverage") && at("make repro-causes") > at("make proof-gate"));
});
