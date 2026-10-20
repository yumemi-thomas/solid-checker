// `verify-delta` is only trustworthy if its mapping is, so the mapping is what
// is tested: every row of the mapping, the longest-prefix rule that keeps
// two rows from stealing each other's paths, and -- the one that matters most --
// the fail-closed fallback, because a wrong "no checks needed" is the failure
// this script could plausibly have.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "vitest";

import {
  BASIS_CAVEATS,
  CHECKS,
  ROWS,
  UNIVERSAL,
  classify,
  planFor,
  baseOf,
  porcelainPaths,
  producerStampDrift,
} from "./verify-delta.mjs";

const withoutUniversal = (checks) => checks.filter((check) => !UNIVERSAL.includes(check));

test("every check a row or the universal set names actually exists", () => {
  for (const row of ROWS) {
    for (const check of row.checks) {
      assert.ok(CHECKS[check], `row ${row.prefix} names an unknown check ${check}`);
    }
  }
  for (const check of [...UNIVERSAL, "build-typefacts", "build-debug", "verify"]) {
    assert.ok(CHECKS[check], `unknown check ${check}`);
    const commands = CHECKS[check].commands ?? [CHECKS[check].command];
    assert.ok(commands.every((command) => Array.isArray(command) && command.length > 0));
  }
});

test("each table row claims its own owner's paths", () => {
  const expected = [
    ["rust/crates/solid-facts/src/lib.rs", ["facts-lib", "wasm-compile"]],
    ["rust/crates/solid-reactive-ir/src/rules/mod.rs", ["ir-lib", "coverage", "wasm-compile"]],
    ["rust/dialects/solid-v2/rules/src/lib.rs", ["dialects-process", "wasm-compile"]],
    ["rust/dialects/solid-v2/compiler/src/lib.rs", ["dialects-process"]],
    ["rust/crates/solid-facts-backend/src/diagnostics.rs", ["backend-process", "coverage"]],
    ["fixtures/reactive-ir/store-flow/App.tsx", ["coverage", "ownership-gate"]],
    ["fixtures/findings-snapshots/engine__oxlint-conformance.json", ["coverage", "ownership-gate"]],
    ["packages/cli/lib/rules-solid-v2.json", ["bun-test-cli"]],
    ["packages/wasm/src/index.ts", ["bun-test-wasm", "wasm-parity"]],
  ];
  for (const [path, checks] of expected) {
    assert.deepEqual(classify(path)?.checks, checks, path);
  }
});

test("the longest matching prefix wins, so no row swallows another's directory", () => {
  // `rust/crates/solid-facts` is a prefix of `rust/crates/solid-facts-backend`
  // as a *string*; only as a path is it not. Matching longest-first is what
  // keeps a backend change from being answered with the facts crate's test.
  assert.deepEqual(classify("rust/crates/solid-facts-backend/src/dialect.rs").checks, [
    "backend-process",
    "coverage",
  ]);
  assert.deepEqual(classify("rust/crates/solid-facts/src/dialect.rs").checks, ["facts-lib", "wasm-compile"]);
});

test("a mapped change set runs preflight checks before building and testing", () => {
  const plan = planFor([
    "rust/crates/solid-reactive-ir/src/lib.rs",
    "fixtures/reactive-ir/store-flow/App.tsx",
  ]);

  assert.equal(plan.full, false);
  assert.deepEqual(plan.unmapped, []);
  assert.deepEqual(plan.checks, [
    // Coverage runs a fresh debug binary: a coverage run against a stale
    // binary would hide the source change under test.
    "build-typefacts",
    ...UNIVERSAL,
    "build-debug",
    "ir-lib",
    "coverage",
    "ownership-gate",
    "wasm-compile",
  ]);
  // No duplicate, even though both rows select coverage.
  assert.equal(plan.checks.filter((check) => check === "coverage").length, 1);
});

test("a change that needs no checker skips the debug build", () => {
  const plan = planFor(["rust/crates/solid-facts/src/ast.rs"]);
  assert.deepEqual(withoutUniversal(plan.checks), ["build-typefacts", "facts-lib", "wasm-compile"]);
  assert.ok(!plan.checks.includes("build-debug"));
});

test("every check a row selects is executed exactly once", () => {
  for (const row of ROWS) {
    const plan = planFor([row.prefix.endsWith("/") ? `${row.prefix}probe` : row.prefix]);
    for (const check of row.checks) {
      assert.equal(plan.checks.filter((id) => id === check).length, 1, `${row.prefix}: ${check}`);
    }
  }
});

test("a WASM package change runs the parity gate once, before the package tests", () => {
  const plan = planFor(["packages/wasm/owned-client.mjs", "packages/wasm/owned-mount.mjs"]);
  assert.equal(plan.full, false);
  assert.deepEqual(plan.unmapped, []);
  assert.deepEqual(plan.checks, ["build-typefacts", ...UNIVERSAL, "wasm-parity", "bun-test-wasm"]);
});

test("analysis-only changes get the WASM compile check, never the package build or parity", () => {
  for (const path of [
    "rust/crates/solid-reactive-ir/src/lib.rs",
    "rust/crates/solid-facts/src/lib.rs",
    "rust/dialects/solid-v2/rules/src/lib.rs",
  ]) {
    const plan = planFor([path]);
    assert.ok(plan.checks.includes("wasm-compile"), path);
    assert.ok(!plan.checks.includes("wasm-parity"), path);
  }
  assert.ok(CHECKS["wasm-compile"].command.includes("--no-default-features"));
  // Host code, the WASM crate and the package keep their own answers.
  for (const path of [
    "rust/crates/solid-facts-backend/src/lib.rs",
    "rust/dialects/solid-v2/compiler/src/lib.rs",
  ]) {
    assert.ok(!planFor([path]).checks.includes("wasm-compile"), path);
  }
  // Parity builds the package, so it replaces the compile check.
  const both = planFor(["rust/crates/solid-facts/src/lib.rs", "packages/wasm/src/index.ts"]);
  assert.ok(both.checks.includes("wasm-parity") && !both.checks.includes("wasm-compile"));
});

test("the universal handoff set precedes tests in every mapped plan", () => {
  for (const row of ROWS) {
    const plan = planFor([row.prefix.endsWith("/") ? `${row.prefix}probe` : row.prefix]);
    assert.deepEqual(plan.checks.slice(1, 1 + UNIVERSAL.length), UNIVERSAL, row.prefix);
  }
  // Nothing changed is still the universal set, never nothing at all.
  assert.deepEqual(planFor([]).checks, ["build-typefacts", ...UNIVERSAL]);
});

test("a path no row claims escalates to the full verify, and says which path did it", () => {
  for (const path of [
    "scripts/coverage.mjs",
    "scripts/lib/pool.mjs",
    "Makefile",
    "schema/typefacts-v1.schema.json",
    "rust/Cargo.toml",
    "rust/Cargo.lock",
    "docs/rules/no-destructure.md",
    "docs/package-contract-v2/compiler-bootstrap/compiler.json",
    "docs/precision-backlog.md.orig",
    "AGENTS.mdx",
    "README.md",
    ".github/workflows/ci.yml",
    "benchmarks/verify-performance.mjs",
    "rust/crates/solid-facts-something-new/src/lib.rs",
  ]) {
    const plan = planFor([path]);
    assert.equal(plan.full, true, `${path} must fail closed`);
    assert.deepEqual(plan.checks, ["verify"]);
    assert.deepEqual(plan.unmapped, [path]);
  }
});

test("prose no check reads runs the universal set only", () => {
  for (const path of [
    "AGENTS.md",
    "CLAUDE.md",
    "CONTEXT.md",
    "CONTRIBUTING.md",
    "docs/precision-backlog.md",
    "docs/decisions.md",
    "docs/open-work.md",
    "docs/README.md",
    "docs/adr/0110-the-checker-analyzes-solid-2-only.md",
    "docs/typefacts/adr/0026-example.md",
    ".claude/skills/add-fixture/SKILL.md",
  ]) {
    const plan = planFor([path]);
    assert.equal(plan.full, false, path);
    assert.deepEqual(plan.checks, ["build-typefacts", ...UNIVERSAL], path);
  }
});

test("the base comes from --base, then SOLID_VERIFY_BASE, then the last green commit, then origin/main", () => {
  assert.deepEqual(baseOf(["bun", "x", "--base", "abc123"], { SOLID_VERIFY_BASE: "env" }), {
    base: "abc123",
  });
  assert.deepEqual(baseOf(["bun", "x"], { SOLID_VERIFY_BASE: "env" }), { base: "env" });
  assert.deepEqual(baseOf(["bun", "x"], {}), { base: "origin/main" });
  // The last green commit sits between the environment and origin/main.
  const green = "a".repeat(40);
  assert.deepEqual(baseOf(["bun", "x"], {}, green), { base: green, green: true });
  assert.deepEqual(baseOf(["bun", "x"], { SOLID_VERIFY_BASE: "env" }, green), { base: "env" });
  assert.deepEqual(baseOf(["bun", "x", "--base", "abc"], {}, green), { base: "abc" });
  assert.ok(baseOf(["bun", "x", "--base"], {}).error);
  assert.ok(baseOf(["bun", "x", "--base", "--dry-run"], {}).error);
});

test("one unmapped path escalates the whole plan, however many are mapped", () => {
  const plan = planFor([
    "rust/crates/solid-facts/src/lib.rs",
    "fixtures/reactive-ir/store-flow/App.tsx",
    "scripts/verify.sh",
  ]);
  assert.equal(plan.full, true);
  assert.deepEqual(plan.checks, ["verify"]);
  assert.deepEqual(plan.unmapped, ["scripts/verify.sh"]);
  // Every path still gets its decision printed -- the mapped ones included, so
  // the reader can see what would have run.
  assert.equal(plan.decisions.length, 3);
});

test("the change set covers both sides of a rename, and NUL records need no unquoting", () => {
  assert.deepEqual(
    porcelainPaths(
      [
        " M scripts/coverage.mjs\0",
        "?? scripts/lib/pool.mjs\0",
        // `-z` puts the new path first, then the original, as separate records.
        "R  fixtures/reactive-ir/new/App.tsx\0",
        "fixtures/reactive-ir/old/App.tsx\0",
        "?? fixtures/reactive-ir/with space/App.tsx\0",
      ].join(""),
    ),
    [
      "scripts/coverage.mjs",
      "scripts/lib/pool.mjs",
      // Both sides: the old path's row still owns whatever stopped existing there.
      "fixtures/reactive-ir/new/App.tsx",
      "fixtures/reactive-ir/old/App.tsx",
      "fixtures/reactive-ir/with space/App.tsx",
    ],
  );
  assert.deepEqual(porcelainPaths(""), []);
});

/**
 * A throwaway git repository under `$TMPDIR`.
 *
 * Not this one: the point is to create paths git has to escape, which must
 * never be written into the real tree.
 */
const withGitRepo = (body) => {
  const root = mkdtempSync(join(tmpdir(), "solid-checker-verify-delta-git-"));
  const git = (...args) => {
    const result = spawnSync("git", args, { cwd: root, encoding: "utf8" });
    assert.equal(result.status, 0, `git ${args.join(" ")}: ${result.stderr}`);
    return result.stdout;
  };
  try {
    git("init", "--quiet");
    git("config", "user.email", "test@example.invalid");
    git("config", "user.name", "test");
    return body({ root, git });
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
};

test("a real non-ASCII path git has to escape is read, not thrown on", () => {
  // The defect this pins: git quotes paths outside a narrow safe set using
  // C-style *octal* escapes -- `"fixtures/reactive-ir/caf\303\251/App.tsx"` --
  // which are not valid JSON escapes. Parsing porcelain with `JSON.parse`
  // therefore dies with an unhandled `SyntaxError` on any non-ASCII changed
  // path, taking the whole plan down. `-z` emits no quoting at all.
  //
  // Note what git does *not* quote: a path containing only a space. So a test
  // built on a hand-written quoted-space line tests a line git never produces,
  // while the line it does produce goes untested. Hence a real repository.
  withGitRepo(({ root, git }) => {
    const names = [
      "fixtures/reactive-ir/café/App.tsx", // non-ASCII: git quotes this one
      "fixtures/reactive-ir/with space/App.tsx", // a space: git does not
      "fixtures/reactive-ir/日本語/App.tsx", // multi-byte, outside Latin-1
      'fixtures/reactive-ir/quote"name/App.tsx', // a literal double quote
      "fixtures/reactive-ir/tab\tname/App.tsx", // a control character
    ];
    for (const name of names) {
      mkdirSync(join(root, name, ".."), { recursive: true });
      writeFileSync(join(root, name), "export const App = () => null;\n");
    }

    // Staged, so porcelain lists each path rather than collapsing the
    // untracked directory they share.
    git("add", "-A");

    // The unquoted form is what a plan must be able to classify.
    const quoted = git("status", "--porcelain");
    assert.match(quoted, /\\303\\251/, "the fixture must actually produce an octal-escaped path");
    assert.throws(
      () => JSON.parse(/"[^\n]*"/.exec(quoted)[0]),
      SyntaxError,
      "the escaped form really is not JSON -- this is the bug being fixed",
    );

    const observed = porcelainPaths(git("status", "--porcelain", "-z"));
    for (const name of names) {
      assert.ok(observed.includes(name), `${JSON.stringify(name)} must survive the parse`);
    }
    // ...and every one of them lands on the fixtures row rather than escalating.
    for (const name of names) {
      assert.deepEqual(classify(name)?.checks, ["coverage", "ownership-gate"], name);
    }
  });
});

test("a real rename of a non-ASCII path contributes both sides", () => {
  withGitRepo(({ root, git }) => {
    mkdirSync(join(root, "fixtures", "reactive-ir", "café"), { recursive: true });
    writeFileSync(join(root, "fixtures/reactive-ir/café/App.tsx"), "export const App = 1;\n");
    git("add", "-A");
    git("commit", "--quiet", "-m", "first");
    git("mv", "fixtures/reactive-ir/café", "fixtures/reactive-ir/naïve");
    const observed = porcelainPaths(git("status", "--porcelain", "-z"));
    assert.ok(observed.includes("fixtures/reactive-ir/naïve/App.tsx"), JSON.stringify(observed));
    assert.ok(observed.includes("fixtures/reactive-ir/café/App.tsx"), JSON.stringify(observed));
  });
});

test("a producer that is not built from the local manifest escalates", () => {
  // `bin/` is gitignored (`.gitignore:1`), so `bin/solid-typefacts` -- the
  // producer of every fact in the repository -- is invisible to both halves of
  // the selection basis. A change set that cannot see it moving must not claim
  // a narrow plan is sufficient.
  const expected = {
    format: 1,
    sourceDigest: "e2f7ac5ce2784f9e4f5bc53f4e100040f6fce3d4".padEnd(64, "0"),
    toolchain: "go-test",
    buildId: "dev",
  };

  assert.equal(
    producerStampDrift({ expected, stamp: JSON.stringify(expected) }),
    null,
    "a matching stamp is not a reason to escalate",
  );
  for (const [what, input] of [
    ["a drifted producer", { expected, stamp: JSON.stringify({ ...expected, sourceDigest: "0".repeat(64) }) }],
    ["an absent stamp", { expected, stamp: null }],
    ["a legacy stamp", { expected, stamp: "build-id=dev" }],
    ["an unreadable manifest", { expected: null, stamp: JSON.stringify(expected) }],
  ]) {
    const drift = producerStampDrift(input);
    assert.equal(typeof drift, "string", `${what} must escalate`);
    assert.ok(drift.length > 20, `${what}: the reason must be a sentence, not a code`);
  }
});

test("the caveats name the input classes the basis cannot see at all", () => {
  // A fail-closed claim with unnamed holes in it is not a fail-closed claim.
  // These are printed on every non-escalating run, so the two classes git
  // cannot report are in front of the reader deciding whether to trust it.
  assert.ok(BASIS_CAVEATS.length >= 2);
  const text = BASIS_CAVEATS.join(" ");
  for (const named of ["bin/solid-typefacts", "rust/target", "node_modules/solid-js", "coverage"]) {
    assert.ok(text.includes(named), `the caveats must name ${named}`);
  }
});
