// `verify-delta` is only trustworthy if its mapping is, so the mapping is what
// is tested: every row of AGENTS.md's table, the longest-prefix rule that keeps
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
    ["rust/crates/solid-facts/src/lib.rs", ["facts-lib"]],
    ["rust/crates/solid-reactive-ir/src/rules/mod.rs", ["ir-lib", "coverage"]],
    ["rust/crates/solid-facts-backend/src/diagnostics.rs", ["backend-process", "coverage"]],
    ["rust/dialects/solid-v1/src/rules.rs", ["contract-process", "conformance"]],
    [
      "pkg/contracts/bundled/solid-v2/solid-js.json",
      ["contract-process", "conformance", "coverage", "ownership-gate"],
    ],
    ["fixtures/reactive-ir/store-flow/App.tsx", ["coverage", "ownership-gate"]],
    ["fixtures/findings-snapshots/engine__oxlint-conformance.json", ["coverage", "ownership-gate"]],
    ["packages/cli/lib/rules-solid-v2.json", ["bun-test-cli"]],
    ["packages/wasm/src/index.ts", ["bun-test-wasm"]],
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
  assert.deepEqual(classify("rust/crates/solid-facts/src/dialect.rs").checks, ["facts-lib"]);
});

test("a mapped change set runs its rows' checks, a fresh binary first, then the universal set", () => {
  const plan = planFor([
    "rust/crates/solid-reactive-ir/src/lib.rs",
    "fixtures/reactive-ir/store-flow/App.tsx",
  ]);

  assert.equal(plan.full, false);
  assert.deepEqual(plan.unmapped, []);
  assert.deepEqual(plan.checks, [
    // AGENTS.md's table says "coverage compare (fresh debug binary)", and a
    // coverage run against a stale binary is the trap the document warns about.
    "build-typefacts",
    "build-debug",
    "ir-lib",
    "coverage",
    "ownership-gate",
    ...UNIVERSAL,
  ]);
  // No duplicate, even though both rows select coverage.
  assert.equal(plan.checks.filter((check) => check === "coverage").length, 1);
});

test("a change that needs no checker skips the debug build", () => {
  const plan = planFor(["rust/crates/solid-facts/src/ast.rs"]);
  assert.deepEqual(withoutUniversal(plan.checks), ["build-typefacts", "facts-lib"]);
  assert.ok(!plan.checks.includes("build-debug"));
});

test("the universal handoff set is appended to every mapped plan", () => {
  for (const row of ROWS) {
    const plan = planFor([`${row.prefix}probe`]);
    assert.deepEqual(plan.checks.slice(-UNIVERSAL.length), UNIVERSAL, row.prefix);
  }
  // Nothing changed is still the universal set, never nothing at all.
  assert.deepEqual(planFor([]).checks, ["build-typefacts", ...UNIVERSAL]);
});

test("a path no row claims escalates to the full verify, and says which path did it", () => {
  for (const path of [
    "scripts/coverage.mjs",
    "scripts/lib/pool.mjs",
    "Makefile",
    "schema/solid-reactivity.schema.json",
    "rust/Cargo.toml",
    "rust/Cargo.lock",
    "AGENTS.md",
    "docs/precision-backlog.md",
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

test("a producer that is not the pinned one escalates, because git cannot report it", () => {
  // `bin/` is gitignored (`.gitignore:1`), so `bin/solid-typefacts` -- the
  // producer of every fact in the repository -- is invisible to both halves of
  // the selection basis. A change set that cannot see it moving must not claim
  // a narrow plan is sufficient.
  const pinned = "e2f7ac5ce2784f9e4f5bc53f4e100040f6fce3d4";
  const other = "0".repeat(40);

  assert.equal(
    producerStampDrift({ pinned, stamp: `revision=${pinned} build-id=dev` }),
    null,
    "a matching stamp is not a reason to escalate",
  );
  for (const [what, input] of [
    ["a drifted producer", { pinned, stamp: `revision=${other} build-id=dev` }],
    ["an absent stamp", { pinned, stamp: null }],
    ["a stamp with no revision", { pinned, stamp: "build-id=dev" }],
    ["an unreadable pin", { pinned: null, stamp: `revision=${pinned} build-id=dev` }],
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
