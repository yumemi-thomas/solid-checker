// `verify-delta` is only trustworthy if its mapping is, so the mapping is what
// is tested: every row of the mapping, the longest-prefix rule that keeps
// two rows from stealing each other's paths, and -- the one that matters most --
// the fail-closed fallback, because a wrong "no checks needed" is the failure
// this script could plausibly have.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "vitest";

import {
  FINDING_GATES,
  ROOT,
  BASIS_CAVEATS,
  CHECKS,
  LANES,
  ROWS,
  UNIVERSAL,
  checkOf,
  classify,
  fixtureKeyOf,
  planFor,
  processTestPins,
  baseOf,
  porcelainPaths,
  producerStampDrift,
  runSchedule,
  scheduleFor,
  wasmParityScope,
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
    ["rust/crates/solid-facts/src/lib.rs", ["facts-lib", "coverage", "repro-causes", "ownership-gate", "tsc-oracle-gate", "obligation-audit", "witnesses", "proof-gate", "wasm-compile"]],
    ["rust/crates/solid-reactive-ir/src/rules/mod.rs", ["ir-lib", "backend-process", "write-walk-strategies", "coverage", "repro-causes", "ownership-gate", "tsc-oracle-gate", "obligation-audit", "witnesses", "proof-gate", "ir-read-lint", "proof-lint", "ir-edit-fuzz", "wasm-compile"]],
    ["rust/dialects/solid-v2/rules/src/lib.rs", ["dialects-process", "coverage", "repro-causes", "ownership-gate", "tsc-oracle-gate", "obligation-audit", "witnesses", "proof-gate", "wasm-compile"]],
    [
      "rust/crates/solid-dialect/src/lib.rs",
      ["dialect-lib", "ir-lib", "backend-process", "backend-staleness", "write-walk-strategies", "dialects-process", "coverage", "repro-causes", "ownership-gate", "tsc-oracle-gate", "obligation-audit", "witnesses", "proof-gate", "ir-edit-fuzz", "wasm-compile"],
    ],
    ["rust/dialects/solid-v2/compiler/src/lib.rs", ["dialects-process", "coverage", "repro-causes", "ownership-gate", "tsc-oracle-gate", "obligation-audit", "witnesses", "proof-gate"]],
    [
      "rust/crates/solid-facts-backend/src/diagnostics.rs",
      ["backend-process", "backend-staleness", "write-walk-strategies", "coverage", "repro-causes", "ownership-gate", "tsc-oracle-gate", "obligation-audit", "witnesses", "proof-gate", "ir-edit-fuzz"],
    ],
    ["fixtures/reactive-ir/store-flow/App.tsx", ["coverage", "ownership-gate", "proof-gate", "guard-count", "backend-process"]],
    ["fixtures/findings-snapshots/engine__oxlint-conformance.json", ["coverage", "ownership-gate", "proof-gate", "guard-count", "backend-process"]],
    [
      "apps/solid-typefacts/src/session.rs",
      ["typefacts", "backend-staleness", "coverage", "repro-causes", "ownership-gate", "tsc-oracle-gate", "obligation-audit", "witnesses", "proof-gate", "ir-edit-fuzz", "wasm-parity"],
    ],
    ["apps/solid-typefacts/tests/process.rs", ["typefacts"]],
    ["apps/solid-typefacts/testdata/forms/tsconfig.json", ["typefacts"]],
    ["packages/cli/lib/rules-solid-v2.json", ["bun-test-cli"]],
    ["packages/wasm/src/index.ts", ["bun-test-wasm", "wasm-parity"]],
    ["scripts/fuzz/focus.mjs", ["fuzz-tests"]],
    ["scripts/fuzz/roles.test.mjs", ["fuzz-tests"]],
    ["scripts/runtime-oracle.mjs", ["fuzz-tests"]],
    ["scripts/verify-delta.mjs", ["verify-delta-script"]],
    ["scripts/verify-delta.test.mjs", ["verify-delta-script"]],
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
    "backend-staleness",
    "write-walk-strategies",
    ...FINDING_GATES,
    "ir-edit-fuzz",
  ]);
  assert.deepEqual(classify("rust/crates/solid-facts/src/dialect.rs").checks, ["facts-lib", ...FINDING_GATES, "wasm-compile"]);
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
    "guard-count",
    "build-debug",
    "ir-lib",
    "backend-process",
    "write-walk-strategies",
    "coverage",
    "repro-causes",
    "ownership-gate",
    "tsc-oracle-gate",
    "obligation-audit",
    "witnesses",
    "proof-gate",
    "ir-read-lint",
    "proof-lint",
    "ir-edit-fuzz",
    "wasm-compile",
  ]);
  // No duplicate, even though both rows select coverage.
  assert.equal(plan.checks.filter((check) => check === "coverage").length, 1);
});

test("a path that can move a finding selects every gate that reads findings", () => {
  // `8833e6bfa`: a merge moved an SC2003 finding, verify-delta selected no gate that read that case, and a red
  // gate was pushed. Each of these paths changes what the checker answers, so none may select a subset.
  for (const path of [
    "rust/crates/solid-facts/src/ast.rs",
    "rust/crates/solid-reactive-ir/src/local_access.rs",
    "rust/crates/solid-facts-backend/src/diagnostics.rs",
    "rust/dialects/solid-v2/rules/src/lib.rs",
    "rust/dialects/solid-v2/compiler/src/lib.rs",
    "rust/crates/solid-dialect/src/dialect.rs",
    "apps/solid-typefacts/src/session.rs",
    "fixtures/tsc-oracle/rule-cases.json",
    "fixtures/obligation-cases/cases.json",
  ]) {
    const plan = planFor([path]);
    assert.equal(plan.full, false, path);
    for (const gate of FINDING_GATES) assert.ok(plan.checks.includes(gate), `${path} must select ${gate}`);
    assert.ok(plan.checks.includes("build-debug"), `${path} must rebuild the checker the gates run`);
  }
  // scripts/verify.sh's gates lane is the authority: a gate it runs on findings is listed here too.
  const lane = readFileSync(join(ROOT, "scripts/verify.sh"), "utf8");
  for (const script of ["coverage.mjs", "uncertifiable-causes.mjs", "ownership-gate.mjs", "tsc-oracle-gate.mjs", "obligation-audit.mjs"]) {
    assert.ok(lane.includes(script), script);
  }
  // Read from the lane, not listed: `repro-causes` reached a push because a list like the one above left it out.
  const gatesLane = lane.slice(lane.indexOf("gates_lane() {"), lane.indexOf("\n}\n", lane.indexOf("gates_lane() {")));
  const laneSteps = [...gatesLane.matchAll(/lane_step ([a-z-]+)/g)].map((match) => match[1]);
  assert.deepEqual(laneSteps, FINDING_GATES);
});

test("a fixture edit selects the proof gate: it runs the checker over every fixture project (ADR 0995)", () => {
  // The laundering: a fixture edited so that a site nothing ends appears passed verify-delta, which ran coverage and the ownership gate only.
  for (const path of ["fixtures/reactive-ir/store-flow/App.tsx", "fixtures/engine/eslint-reactivity-v2/App.tsx", "fixtures/uncertifiable-repro/sc2001/src/after-await.tsx", "fixtures/proof-log/candidates.json"]) {
    const plan = planFor([path]);
    assert.equal(plan.full, false, path);
    assert.ok(plan.checks.includes("proof-gate"), `${path} must select proof-gate`);
    assert.ok(plan.checks.includes("build-debug"), `${path} must rebuild the checker the gate runs`);
  }
  // The held cases, HELD_CAP and the frozen judge reach the base checks.
  for (const path of ["scripts/uncertifiable-causes.mjs", "scripts/fuzz/judge-frozen.json", "fixtures/uncertifiable-repro/cases.json"]) {
    assert.ok(planFor([path]).checks.includes("guard-count"), path);
  }
});

test("a change that needs no checker skips the debug build", () => {
  const plan = planFor(["packages/cli/launcher.mjs"]);
  assert.deepEqual(withoutUniversal(plan.checks), ["build-typefacts", "bun-test-cli"]);
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
    "rust/crates/solid-dialect/src/lib.rs",
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

test("a rule page reaches what reads it, and `.gitignore` what asks git", () => {
  const page = planFor(["docs/rules/no-destructure.md"]);
  assert.equal(page.full, false);
  assert.deepEqual(withoutUniversal(page.checks), ["build-typefacts", "rule-pages"]);
  const ignored = planFor([".gitignore"]);
  assert.equal(ignored.full, false);
  assert.ok(["coverage", "repro-budget"].every((check) => ignored.checks.includes(check)));
  // One exact file, and a name inside docs/rules/ only.
  assert.equal(planFor([".gitignore.bak"]).full, true);
  assert.equal(planFor(["docs/rules.md"]).full, true);
});

test("the flip's notes are prose, and its tools reach their own check", () => {
  for (const path of ["scripts/flip/notes/track-r-progress.md", "scripts/flip/PLAN.md"]) {
    const plan = planFor([path]);
    assert.equal(plan.full, false, path);
    assert.deepEqual(withoutUniversal(plan.checks), ["build-typefacts"]);
  }
  for (const path of ["scripts/flip/check.mjs", "scripts/flip/witnesses.json"]) {
    const plan = planFor([path]);
    assert.equal(plan.full, false, path);
    assert.deepEqual(withoutUniversal(plan.checks), ["build-typefacts", "flip-tools", "build-debug", "witnesses"]);
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
      assert.deepEqual(classify(name)?.checks, ["coverage", "ownership-gate", "proof-gate", "guard-count", "backend-process"], name);
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
    toolchain: "rustc-test",
    buildId: "dev",
  };

  assert.equal(
    producerStampDrift({ expected, stamp: JSON.stringify(expected) }),
    null,
    "a matching stamp is not a reason to escalate",
  );
  // The thin-LTO inner-loop producer and the release one are the same sources
  // through the same compiler, so neither drifts from the other.
  assert.equal(
    producerStampDrift({ expected: { ...expected, profile: "bench-dev" }, stamp: JSON.stringify(expected) }),
    null,
  );
  assert.equal(
    producerStampDrift({ expected, stamp: JSON.stringify({ ...expected, profile: "bench-dev" }) }),
    null,
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

test("the differential harness's own files select its tests; its neighbours stay unmapped", () => {
  const plan = planFor(["scripts/typefacts-differential.mjs", "rust/crates/typefacts/src/transcript.rs"]);
  assert.equal(plan.full, false);
  assert.ok(plan.checks.includes("differential-harness"));
  assert.ok(!plan.checks.includes("coverage"));
  for (const path of [
    "scripts/typefacts-transcript-proxy.mjs",
    "rust/crates/typefacts/src/lib.rs",
    "rust/crates/typefacts/Cargo.toml",
  ]) {
    assert.equal(planFor([path]).full, true, `${path} must fail closed`);
  }
});

test("every check has a lane, and one lane owns each target directory", () => {
  // AGENTS.md: one Cargo process per target directory. Lanes run side by side,
  // so the lane is what keeps two builds out of one directory.
  const owner = new Map();
  for (const [check, definition] of Object.entries(CHECKS)) {
    assert.ok(LANES[check], `${check} has no lane`);
    if (check === "verify" || check === "wasm-parity") continue;
    for (const command of definition.commands ?? [definition.command]) {
      if (command[0] !== "cargo" || ["fmt"].includes(command[2])) continue;
      const explicit = command.indexOf("--target-dir");
      const directory = explicit !== -1 ? command[explicit + 1]
        : command.includes("apps/solid-typefacts/Cargo.toml") ? "apps/solid-typefacts/target" : "rust/target";
      const lane = owner.get(directory) ?? LANES[check];
      assert.equal(LANES[check], lane, `${check} builds in ${directory}, which the ${lane} lane owns`);
      owner.set(directory, lane);
    }
  }
});

test("the schedule runs the producer first, preflight next, and gates after the build they run", () => {
  const plan = planFor(["rust/crates/solid-reactive-ir/src/lib.rs", "apps/solid-typefacts/src/main.rs"]);
  const schedule = new Map(scheduleFor(plan.checks).map((entry) => [entry.check, entry]));
  assert.deepEqual(schedule.get("build-typefacts").after, []);
  for (const entry of schedule.values()) {
    if (entry.check !== "build-typefacts") assert.ok(entry.after.includes("build-typefacts"), entry.check);
    if (!["producer", "preflight"].includes(entry.lane)) {
      assert.ok(entry.after.includes("fmt-check"), `${entry.check} must wait for the cheap preflight`);
    }
  }
  for (const prior of ["build-debug", "ir-lib"]) {
    assert.ok(schedule.get("coverage").after.includes(prior), `coverage must wait for ${prior}`);
  }
  for (const prior of ["clippy", "build-debug", "ir-lib", "typefacts"]) {
    assert.ok(schedule.get("wasm-parity").after.includes(prior), `wasm-parity must wait for ${prior}`);
  }
});

const nodeCheck = (code) => [[process.execPath, "-e", code]];

test("lanes run side by side, and a lane runs its own checks in order", async () => {
  const directory = mkdtempSync(join(tmpdir(), "verify-delta-lanes-"));
  try {
    const marker = (name) => JSON.stringify(join(directory, name));
    // `a` finishes only once `b` has started: it would time out if the lanes ran
    // one after the other. `c` shares a's lane, so `a` must be done before it.
    const commands = {
      a: nodeCheck(`const fs=require('node:fs');const t=setInterval(()=>{if(fs.existsSync(${marker("b")})){fs.writeFileSync(${marker("a")},'');process.exit(0)}},10);setTimeout(()=>process.exit(9),5000)`),
      b: nodeCheck(`require('node:fs').writeFileSync(${marker("b")},'')`),
      c: nodeCheck(`process.exit(require('node:fs').existsSync(${marker("a")})?0:8)`),
    };
    const failure = await runSchedule(
      [{ check: "a", lane: "one", after: [] }, { check: "b", lane: "two", after: [] }, { check: "c", lane: "one", after: [] }],
      { commandsOf: (check) => commands[check], environmentOf: () => process.env },
    );
    assert.equal(failure, null);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("a failure stops the run: nothing waiting on it starts, and running checks are stopped", async () => {
  const directory = mkdtempSync(join(tmpdir(), "verify-delta-failure-"));
  try {
    const started = join(directory, "dependent-started");
    const commands = {
      broken: nodeCheck("process.exit(3)"),
      dependent: nodeCheck(`require('node:fs').writeFileSync(${JSON.stringify(started)},'')`),
      slow: nodeCheck("setTimeout(() => process.exit(0), 20000)"),
    };
    const begun = Date.now();
    const failure = await runSchedule(
      [
        { check: "slow", lane: "other", after: [] },
        { check: "broken", lane: "one", after: [] },
        { check: "dependent", lane: "two", after: ["broken"] },
      ],
      { commandsOf: (check) => commands[check], environmentOf: () => process.env },
    );
    assert.equal(failure.check, "broken");
    assert.equal(failure.status, 3);
    assert.equal(existsSync(started), false);
    assert.ok(Date.now() - begun < 10000, "the slow check must have been stopped");
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("make verify skips WASM parity only for analysis-only or prose change sets", () => {
  const scope = (...paths) => wasmParityScope({ paths });
  for (const paths of [
    ["rust/crates/solid-reactive-ir/src/pipeline.rs"],
    ["rust/crates/solid-facts/src/lib.rs", "rust/dialects/solid-v2/rules/src/lib.rs", "docs/decisions.md"],
    ["rust/crates/solid-dialect/src/lib.rs", "rust/crates/solid-dialect/clippy.toml"],
  ]) {
    assert.equal(scope(...paths).run, false, paths.join(", "));
  }
  for (const paths of [
    // The WASM crate links the backend, so its changes keep parity.
    ["rust/crates/solid-reactive-ir/src/pipeline.rs", "rust/crates/solid-facts-backend/src/lib.rs"],
    ["rust/crates/solid-dialect/src/lib.rs", "rust/crates/solid-facts-backend/src/lib.rs"],
    ["rust/dialects/solid-v2/compiler/src/lib.rs"],
    ["fixtures/reactive-ir/a/App.tsx"],
    ["packages/wasm/src/index.mjs"],
    ["apps/solid-typefacts/src/main.rs"],
    ["scripts/verify.sh"],
    ["rust/Cargo.toml"],
  ]) {
    const result = scope(...paths);
    assert.equal(result.run, true, paths.join(", "));
    assert.ok(result.reason.length > 20);
  }
  // Fail closed: an unreadable or empty change set runs it.
  assert.equal(wasmParityScope({ error: "cannot diff against origin/main" }).run, true);
  assert.equal(wasmParityScope({ paths: [] }).run, true);
});

test("each analysis crate whose changes skip WASM parity confines threads with the lint", () => {
  for (const row of ROWS.filter((candidate) => candidate.checks.includes("wasm-compile"))) {
    const config = readFileSync(join(row.prefix, "clippy.toml"), "utf8");
    for (const method of ["std::thread::scope", "std::thread::spawn", "std::thread::Scope::spawn"]) {
      assert.ok(config.includes(`"${method}"`), `${row.prefix}clippy.toml must disallow ${method}`);
    }
  }
});

/**
 * The workspace's crates: `{ name, directory, dependencies }` from `rust/Cargo.toml`'s members and each member's manifest (the names under its
 * dependency tables; a line `name = ...` or `name.workspace = true`). A scan, not a TOML parser: the manifests here are flat.
 */
function workspaceCrates() {
  const members = [...readFileSync(join(ROOT, "rust/Cargo.toml"), "utf8").match(/members\s*=\s*\[([^\]]*)\]/)[1].matchAll(/"([^"]+)"/g)].map((match) => match[1]);
  return members.map((member) => {
    const manifest = readFileSync(join(ROOT, "rust", member, "Cargo.toml"), "utf8");
    const name = manifest.match(/^name\s*=\s*"([^"]+)"/m)[1];
    const dependencies = new Set();
    let table = "";
    for (const line of manifest.split("\n")) {
      const header = line.match(/^\[([^\]]+)\]/);
      if (header) { table = header[1]; continue; }
      if (!/(^|\.)(dependencies|dev-dependencies|build-dependencies)$/.test(table)) continue;
      const entry = line.match(/^([A-Za-z0-9_-]+)\s*(=|\.)/);
      if (entry) dependencies.add(entry[1]);
    }
    return { name, directory: `rust/${member}/`, dependencies };
  });
}

// `rust/crates/solid-dialect/` was unmapped on purpose, because a row narrower than the crate's blast radius is a guess. The row is the radius, so it
// is derived from the workspace's dependency graph and pinned to it: a crate that starts to depend on solid-dialect fails here until its own tests
// are named in the row.
test("the dialect row follows the workspace's dependency graph", () => {
  const crates = workspaceCrates();
  const direct = crates.filter((crate) => crate.dependencies.has("solid-dialect")).map((crate) => crate.name).sort();
  // Each direct dependent's own tests, as the rows of its directory name them.
  const ownTests = {
    "solid-reactive-ir": ["ir-lib"],
    "solid-facts-backend": ["backend-process", "backend-staleness", "write-walk-strategies"],
    "solid-v2-rules": ["dialects-process"],
  };
  assert.deepEqual(direct, Object.keys(ownTests).sort(), "a crate that depends on solid-dialect needs its tests named here and in the row");
  const row = classify("rust/crates/solid-dialect/src/lib.rs");
  for (const [name, checks] of Object.entries(ownTests)) {
    const dependent = crates.find((crate) => crate.name === name);
    const own = classify(`${dependent.directory}src/lib.rs`);
    for (const check of checks) {
      assert.ok(row.checks.includes(check), `${name}'s ${check} must be in the dialect row`);
      assert.ok(own.checks.includes(check), `${name}'s own row names ${check}`);
    }
  }
  // The closure: what depends on a dependent depends on the dialect. Today only the WASM crate (through the backend), which the compile check covers.
  const closure = new Set(direct);
  for (let grew = true; grew;) {
    grew = false;
    for (const crate of crates) {
      if (!closure.has(crate.name) && [...closure].some((name) => crate.dependencies.has(name))) { closure.add(crate.name); grew = true; }
    }
  }
  assert.deepEqual([...closure].filter((name) => !(name in ownTests)).sort(), ["solid-checker-wasm"]);
  assert.ok(row.checks.includes("wasm-compile") && !row.checks.includes("wasm-parity"));
  // The crate's own tests, every gate that reads findings, and the daemon against the one-shot run; none of the facts crate's (it does not depend on this one).
  for (const check of ["dialect-lib", ...FINDING_GATES, "ir-edit-fuzz"]) assert.ok(row.checks.includes(check), check);
  assert.ok(!row.checks.includes("facts-lib"));
  assert.ok(!crates.find((crate) => crate.name === "solid-facts").dependencies.has("solid-dialect"));
  assert.deepEqual(CHECKS["dialect-lib"].command.slice(-3), ["-p", "solid-dialect", "--lib"]);
});

test("a change to the dialect crate runs its tests first, then the dependents' and the gates, without the WASM package", () => {
  const plan = planFor(["rust/crates/solid-dialect/src/solid_2.rs", "rust/crates/solid-dialect/clippy.toml"]);
  assert.equal(plan.full, false);
  assert.deepEqual(plan.unmapped, []);
  assert.deepEqual(plan.checks, [
    "build-typefacts",
    ...UNIVERSAL,
    "build-debug",
    "dialect-lib",
    "ir-lib",
    "backend-process",
    "backend-staleness",
    "write-walk-strategies",
    "dialects-process",
    ...FINDING_GATES,
    "ir-edit-fuzz",
    "wasm-compile",
  ]);
  assert.equal(scheduleFor(plan.checks).find((entry) => entry.check === "dialect-lib").lane, "rust");
  assert.equal(wasmParityScope({ paths: ["rust/crates/solid-dialect/src/solid_2.rs"] }).run, false);
  // The crate's thread lint is what lets its changes skip parity (the test above reads it for every such row).
  assert.ok(existsSync(join(ROOT, "rust/crates/solid-dialect/clippy.toml")));
});

test("every entry of the Solid 2 dialect directory has a row, so a change there never escalates", () => {
  const directory = "rust/dialects/solid-v2";
  const entries = readdirSync(join(ROOT, directory));
  assert.ok(entries.length >= 4);
  for (const entry of entries) {
    const path = statSync(join(ROOT, directory, entry)).isDirectory() ? `${directory}/${entry}/src/lib.rs` : `${directory}/${entry}`;
    assert.ok(classify(path), `${path} has no row`);
    assert.equal(planFor([path]).full, false, path);
  }
});

test("a change to the mapping or to its test selects the test, never the full verify", () => {
  const plan = planFor(["scripts/verify-delta.mjs", "scripts/verify-delta.test.mjs"]);
  assert.equal(plan.full, false);
  assert.deepEqual(plan.checks, ["build-typefacts", ...UNIVERSAL, "verify-delta-script"]);
  assert.deepEqual(checkOf("verify-delta-script").command.slice(-1), ["scripts/verify-delta.test.mjs"]);
  assert.equal(scheduleFor(["verify-delta-script"])[0].lane, "preflight");
});

// A process test that runs the checker on a fixture and asserts its rows moves with that fixture's snapshot: 1113 changed
// after-await-net's SC1002 rows, verify-delta ran only `backend-process` (diagnostics_process), and CI's
// after_await_accounting_process failed.
test("a fixture's snapshot or source selects every process test that pins it", () => {
  assert.equal(fixtureKeyOf("fixtures/findings-snapshots/reactive-ir__after-await-net.json"), "reactive-ir/after-await-net");
  assert.equal(fixtureKeyOf("fixtures/reactive-ir/after-await-net/App.tsx"), "reactive-ir/after-await-net");
  assert.equal(fixtureKeyOf("fixtures/tsc-oracle/rule-cases.json"), null);
  assert.equal(fixtureKeyOf("rust/crates/solid-facts/src/lib.rs"), null);

  const plan = planFor(["fixtures/findings-snapshots/reactive-ir__after-await-net.json"]);
  const pinned = plan.checks.filter((check) => check.includes("after_await_accounting_process"));
  assert.equal(pinned.length, 1);
  assert.deepEqual(checkOf(pinned[0]).command.slice(-6), ["-p", "solid-facts-backend", "--test", "backend_process", "--", "after_await_accounting_process::"]);
  assert.equal(scheduleFor(plan.checks).find((entry) => entry.check === pinned[0]).lane, "rust");
  // A target that needs a feature runs through that feature's own make target, never a `--features` flag (ADR 0950): a fixture
  // what_if_process pins selects `make test-what-if`; `diagnostics_process` is `backend-process` itself.
  const pins = processTestPins();
  const whatIf = pins.find((pin) => pin.test === "what_if_process");
  assert.equal(whatIf?.check, "test-what-if");
  assert.ok(whatIf.fixtures.has("reactive-ir/stored-function-after-await"));
  assert.ok(planFor(["fixtures/findings-snapshots/reactive-ir__stored-function-after-await.json"]).checks.includes("test-what-if"));
  assert.deepEqual(checkOf("test-what-if").command, ["make", "test-what-if"]);
  assert.equal(scheduleFor(["test-what-if"])[0].lane, "rust");
  assert.ok(!pins.some((pin) => checkOf(pin.check).command.join(" ").includes("--features")));
  assert.ok(pins.filter((pin) => pin.test === "diagnostics_process").every((pin) => pin.check === "backend-process"));
  // A fixture no process test names selects none.
  const synthetic = planFor(["fixtures/reactive-ir/unpinned-probe/App.tsx"], { pins: [{ check: "process-test:p|t|", fixtures: new Set(["reactive-ir/other"]) }] });
  assert.ok(!synthetic.checks.some((check) => check.startsWith("process-test:")));
  const named = planFor(["fixtures/reactive-ir/other/App.tsx"], { pins: [{ check: "process-test:p|t|", fixtures: new Set(["reactive-ir/other"]) }] });
  assert.ok(named.checks.includes("process-test:p|t|"));
  assert.deepEqual(checkOf("process-test:p|t|").command.slice(-4), ["-p", "p", "--test", "t"]);
});

