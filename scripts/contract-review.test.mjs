import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import test from "node:test";

const root = resolve(import.meta.dirname, "..");
const native = process.env.SOLID_CHECKER_NATIVE_BIN ?? join(root, "rust/target/debug/solid-checker-rust");
const typeFacts = process.env.SOLID_TYPEFACTS_BIN ?? join(root, "bin/solid-typefacts");
const canRun = existsSync(native) && existsSync(typeFacts);

test("contract generate writes a one-line summary and sibling review plan", { skip: !canRun }, () => {
  const temporary = mkdtempSync(join(tmpdir(), "solid-checker-contract-review-"));
  const output = join(temporary, "solid-reactivity.json");
  try {
    const result = spawnSync(
      process.execPath,
      [
        join(root, "packages/cli/bin/solid-checker.mjs"),
        "contract",
        "generate",
        "--package-root",
        join(root, "fixtures/package-contracts/shorthand-block-scope"),
        "--output",
        output
      ],
      {
        cwd: root,
        env: {
          ...process.env,
          SOLID_CHECKER_NATIVE_BIN: native,
          SOLID_TYPEFACTS_BIN: typeFacts
        },
        encoding: "utf8"
      }
    );
    assert.equal(result.status, 0, result.stderr);
    assert.equal(result.stdout.trim().split(/\r?\n/).length, 1, result.stdout);
    assert.match(result.stdout, /review plan .*\.review\.md/);
    const review = readFileSync(join(temporary, "solid-reactivity.review.md"), "utf8");
    for (const section of [
      "## exports with no summary",
      "## callbacks with no execution row",
      "## inherited rows",
      "## environment-branching exports"
    ]) {
      assert.match(review, new RegExp(section));
    }
    assert.match(review, /generated evidence is inferred/i);
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }
});
