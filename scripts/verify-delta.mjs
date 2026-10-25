#!/usr/bin/env bun
// `make verify-delta`: maps changed paths to the checks that own them.
//
// The table tells a reader which narrow check a change needs and which
// expensive ones can wait. Applied by hand it is a judgement call made under
// time pressure, which is the condition under which the wrong row gets picked.
// This runs the table instead: it reads what actually changed, prints the row
// it matched for every path, and runs the universal preflight before the
// selected tests and gates.
//
//   bun scripts/verify-delta.mjs              plan and run
//   bun scripts/verify-delta.mjs --dry-run    print the plan, run nothing
//   bun scripts/verify-delta.mjs --base REV   diff against REV (or SOLID_VERIFY_BASE;
//                                             `make verify-delta BASE=REV`)
//
// Without an explicit base, the plan diffs against the last commit a full
// `make verify` passed on with a clean tree (rust/target/verify-green-commit,
// written by scripts/verify.sh) when that commit is an ancestor of HEAD, and
// otherwise against origin/main. Everything since the last green run is
// planned; nothing before it needs re-checking.
//
// **`make verify` remains the handoff authority.** This is the fast loop, and
// it is only ever as good as the mapping below; a full run is what a claim of
// "verified" rests on.
//
// The mapping fails closed. A path that matches no row -- `scripts/`, the
// `Makefile`, `schema/`, `rust/Cargo.toml`, `docs/rules/`, anything new --
// escalates to the full `make verify`, and prints which path did it. An
// unmapped path can change any answer in the repository, so guessing that it
// changes none is the one failure mode this script must not have.
//
// **What the selection basis cannot see, and what is done about it.** The basis
// is git: a merge-base diff plus the working tree. Anything git ignores is
// therefore invisible to it, and two ignored classes are real inputs:
//
//   1. The build products under `/bin/` and `rust/target/` -- above all
//      `bin/solid-typefacts`, the producer of every fact in the repository.
//      Rebuilding it changes every answer while `git status` stays silent, so
//      `build-typefacts` (a local source-manifest stamp check) is in *every*
//      plan, and a stamp that does not match Type Facts-owned inputs escalates
//      the whole plan.
//   2. Ignored fixture inputs -- notably a `node_modules/solid-js` dialect stub
//      added to an already-tracked fixture without its `.gitignore` exception.
//      `git status` cannot see it, so no row selects `coverage`, and
//      `checkDialectStubs` (the check that catches a silently substituted
//      dialect) lives inside `coverage`. This one is *not* closed; it is printed
//      as a caveat on every run, because closing it means hashing the fixture
//      trees, which is `coverage`'s own job.
import { spawn, spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";
import { identity as typefactsIdentity } from "./typefacts-source-identity.mjs";
import { certificationEnvironment } from "./lib/certification-environment.mjs";

export const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");

const TOOLCHAIN = "+1.99.1";
const MANIFEST = "rust/Cargo.toml";
const TYPEFACTS_MANIFEST = "apps/solid-typefacts/Cargo.toml";
const DEBUG_CHECKER = "rust/target/debug/solid-checker-rust";
const TYPEFACTS = "bin/solid-typefacts";
// Check-only builds (clippy, the WASM compile check) have a target directory of
// their own, which scripts/verify.sh shares, so they compile beside the debug
// build and the tests rather than queueing behind them on `rust/target`.
const LINT_TARGET = "rust/target/lint";

/**
 * The checks, by identifier.
 *
 * Commands carry the toolchain pin and
 * the `SOLID_TYPEFACTS_BIN` arming that fixture-driven process tests skip
 * silently without.
 */
export const CHECKS = {
  "build-typefacts": {
    why:
      "the producer of every fact is gitignored, so `git status` cannot report it moving; " +
      "this source-manifest stamp check no-ops only when the ignored binary matches every local Type Facts input",
    command: ["scripts/build-typefacts.sh"],
  },
  "build-debug": {
    why:
      "coverage and the ownership gate must run a fresh debug binary, not a packaged one that can lag " +
      "rust/ source",
    command: [
      "cargo", TOOLCHAIN, "build", "--manifest-path", MANIFEST,
      "-p", "solid-facts-backend", "--bin", "solid-checker-rust",
    ],
  },
  "facts-lib": {
    command: ["cargo", TOOLCHAIN, "test", "--manifest-path", MANIFEST, "-p", "solid-facts", "--lib"],
  },
  "ir-lib": {
    command: [
      "cargo", TOOLCHAIN, "test", "--manifest-path", MANIFEST, "-p", "solid-reactive-ir", "--lib",
    ],
  },
  "backend-process": {
    command: [
      "cargo", TOOLCHAIN, "test", "--manifest-path", MANIFEST, "-p", "solid-facts-backend",
      "--test", "diagnostics_process",
    ],
    env: { SOLID_TYPEFACTS_BIN: join(ROOT, TYPEFACTS) },
  },
  "backend-staleness": {
    why:
      "a slice that breaks the cached-answer digest, the resolution witness, the kept attestation or pass " +
      "reuse answers a retained edit from stale state; these process tests compare such edits with fresh runs",
    command: [
      "cargo", TOOLCHAIN, "test", "--manifest-path", MANIFEST, "-p", "solid-facts-backend",
      "--test", "backend_process", "--",
      "incremental_staleness_process::", "attestation_kept_process::", "daemon_residue_process::",
      "idle_shadow_process::",
    ],
    env: { SOLID_TYPEFACTS_BIN: join(ROOT, TYPEFACTS) },
  },
  "dialects-process": {
    command: [
      "cargo", TOOLCHAIN, "test", "--manifest-path", MANIFEST, "-p", "solid-facts-backend",
      "--test", "dialects_process",
    ],
    env: { SOLID_TYPEFACTS_BIN: join(ROOT, TYPEFACTS) },
  },
  coverage: {
    command: ["bun", "scripts/coverage.mjs"],
    env: {
      SOLID_CHECKER_BIN: join(ROOT, DEBUG_CHECKER),
      SOLID_TYPEFACTS_BIN: join(ROOT, TYPEFACTS),
    },
  },
  "ownership-gate": {
    command: ["bun", "scripts/ownership-gate.mjs", "--require-retained", "--require-complete"],
    env: {
      SOLID_CHECKER_BIN: join(ROOT, DEBUG_CHECKER),
      SOLID_TYPEFACTS_BIN: join(ROOT, TYPEFACTS),
    },
  },
  "ir-read-lint": {
    why:
      "a new raw cross-file read in the Reactive IR is a read an incremental build cannot see; " +
      "the lint ratchets the allow-listed sites down (scripts/ir-read-lint.mjs)",
    command: ["bun", "scripts/ir-read-lint.mjs"],
  },
  "ir-edit-fuzz": {
    why:
      "the retained daemon's answer must equal a fresh one after random edits (and shadow mode must stay " +
      "silent); this is the seconds-long fixture budget, on a copy of each fixture",
    commands: [
      ["bun", "scripts/ir-edit-fuzz.mjs", "--fixtures", "--steps", "6"],
      ["bun", "scripts/ir-edit-fuzz.mjs", "--fixtures", "--family", "retype", "--steps", "4"],
    ],
    env: {
      SOLID_CHECKER_BIN: join(ROOT, DEBUG_CHECKER),
      SOLID_TYPEFACTS_BIN: join(ROOT, TYPEFACTS),
    },
  },
  "ir-tools-test": {
    why: "the lint and the harness are scripts with their own unit tests",
    command: [
      "bun", "packages/cli/node_modules/vitest/vitest.mjs", "run",
      "--config", "packages/cli/vitest.config.mjs",
      "scripts/ir-read-lint.test.mjs", "scripts/ir-edit-fuzz.test.mjs",
    ],
  },
  "bun-test-cli": { command: ["bun", "run", "--cwd", "packages/cli", "test"] },
  "bun-test-wasm": { command: ["bun", "run", "--cwd", "packages/wasm", "test"] },
  "wasm-parity": {
    why:
      "the owned check ships two reactors built from rust/ and apps/ beside the package's own " +
      "worker and transport; only the packed package, installed in isolation and compared with the " +
      "native check, shows a package change kept that answer. It goes through the Makefile, which " +
      "builds the package and records the update frames before the gate, which builds nothing",
    command: ["make", "wasm-parity"],
  },
  "wasm-compile": {
    why:
      "an analysis-only change moves the native and WASM builds of the same analysis identically, so the " +
      "package build and parity gate are skipped; this seconds-long check keeps a WASM-only compile break " +
      "(the crate without its default napi host) failing here rather than only in `make verify`",
    command: [
      "cargo", TOOLCHAIN, "check", "--target-dir", LINT_TARGET, "--manifest-path", MANIFEST,
      "-p", "solid-checker-wasm", "--all-targets", "--no-default-features", "--features", "dialect-v2",
    ],
  },
  "typefacts": {
    why:
      "the Type Facts producer is its own Cargo workspace (own lockfile and target), so the rust/ " +
      "fmt and clippy checks never see it; its differential tests skip unless a reference producer is available",
    commands: [
      ["cargo", TOOLCHAIN, "fmt", "--manifest-path", TYPEFACTS_MANIFEST, "--", "--check"],
      [
        "cargo", TOOLCHAIN, "clippy", "--manifest-path", TYPEFACTS_MANIFEST, "--all-targets",
        "--", "-D", "warnings",
      ],
      [
        "cargo", TOOLCHAIN, "clippy", "--manifest-path", TYPEFACTS_MANIFEST, "--target", "wasm32-wasip1",
        "--features", "reactor", "--lib", "--", "-D", "warnings",
      ],
      ["cargo", TOOLCHAIN, "test", "--manifest-path", TYPEFACTS_MANIFEST],
    ],
  },
  "differential-harness": {
    why:
      "the Type Facts differential harness (recorded transcripts of a reference producer replayed on a candidate) " +
      "is test tooling: its JS normalisation and diff logic, and the transcript decoder it shells out to",
    commands: [
      [
        "bun", "packages/cli/node_modules/vitest/vitest.mjs", "run",
        "--config", "packages/cli/vitest.config.mjs", "scripts/typefacts-differential.test.mjs",
      ],
      ["cargo", TOOLCHAIN, "test", "--manifest-path", MANIFEST, "-p", "typefacts", "--lib", "transcript"],
    ],
  },
  // The universal handoff set, appended to every plan.
  "fmt-check": {
    command: ["cargo", TOOLCHAIN, "fmt", "--manifest-path", MANIFEST, "--all", "--", "--check"],
  },
  "whitespace-check": { command: ["git", "diff", "--check"] },
  "schema-json": { command: ["jq", "empty", "schema/typefacts-v1.schema.json", "schema/typefacts-codec-limits.json"] },
  "dialect-manifests": { command: ["bun", "scripts/dialect-manifests.mjs", "validate"] },
  clippy: {
    command: [
      "cargo", TOOLCHAIN, "clippy", "--target-dir", LINT_TARGET, "--manifest-path", MANIFEST,
      "--workspace", "--all-targets", "--", "-D", "warnings",
    ],
  },
  verify: { command: ["scripts/verify.sh"] },
};

/** The universal handoff set, run before expensive tests and gates. */
export const UNIVERSAL = [
  "fmt-check",
  "whitespace-check",
  "schema-json",
  "dialect-manifests",
  "clippy",
];

/**
 * One row of the mapping, as a prefix test.
 *
 * Prefixes are matched longest-first, so `rust/crates/solid-facts-backend/`
 * cannot be swallowed by `rust/crates/solid-facts`. Nothing here matches by
 * name fragment or regex: a row applies to a directory, exactly.
 */
export const ROWS = [
  // The three analysis-only rows (solid-facts, solid-reactive-ir, the v2 rules
  // crate) select `wasm-compile` and never the WASM package build or parity:
  // parity compares two builds of one analysis, so an edit confined to the
  // analysis moves both identically. Every other rust/ row, and anything
  // under packages/wasm, solid-checker-wasm, typefacts, apps/ or
  // scripts/wasm-parity*, keeps its own answer; `make verify` is the authority.
  {
    prefix: "rust/crates/solid-facts/",
    owner: "solid-facts (AST, normalized facts)",
    checks: ["facts-lib", "wasm-compile"],
  },
  {
    prefix: "rust/crates/solid-reactive-ir/",
    owner: "solid-reactive-ir (IR, indexes, source summaries, interprocedural, rules engine)",
    checks: ["ir-lib", "coverage", "ir-read-lint", "ir-edit-fuzz", "wasm-compile"],
  },
  {
    prefix: "rust/crates/solid-facts-backend/",
    owner: "solid-facts-backend process/diagnostics",
    checks: ["backend-process", "backend-staleness", "coverage", "ir-edit-fuzz"],
  },
  // No `rust/crates/solid-dialect/` row, deliberately. The crate owns the
  // shared `Dialect` interface that `solid-reactive-ir` and the dialect crate
  // consume -- so a change there can move findings, break the IR library
  // tests, and break the backend's process tests alike. Writing a row narrower
  // than that blast radius is exactly the guess this script must not make, so
  // the path is unmapped and escalates.
  {
    prefix: "rust/dialects/",
    owner: "dialects at the process boundary",
    checks: ["dialects-process"],
  },
  {
    prefix: "rust/dialects/solid-v2/rules/",
    owner: "the Solid 2 rules crate (analysis only)",
    checks: ["dialects-process", "wasm-compile"],
  },
  {
    prefix: "fixtures/",
    owner: "fixtures or expected findings",
    checks: ["coverage", "ownership-gate"],
  },
  // The producer is the default (ADR 0365): a source change can move any fact,
  // so it also selects the fixture gates and the WASM parity gate that runs the
  // same code as a reactor. Its tests and testdata only feed its own tests.
  {
    prefix: "apps/solid-typefacts/",
    owner: "the Type Facts producer (separate workspace; the default producer)",
    checks: ["typefacts", "backend-staleness", "coverage", "ownership-gate", "ir-edit-fuzz", "wasm-parity"],
  },
  {
    prefix: "apps/solid-typefacts/tests/",
    owner: "the Type Facts producer's tests",
    checks: ["typefacts"],
  },
  {
    prefix: "apps/solid-typefacts/testdata/",
    owner: "the Type Facts producer's test projects",
    checks: ["typefacts"],
  },
  // The differential harness's own files, one exact path each. Neighbours
  // (scripts/coverage.mjs, the proxy, the Makefile, the typefacts crate's
  // manifest and lib.rs) stay unmapped on purpose: they feed other answers too.
  ...[
    "scripts/typefacts-differential.mjs",
    "scripts/typefacts-differential.test.mjs",
    "scripts/lib/typefacts-differential.mjs",
    "scripts/typefacts-differential-allowlist.json",
    "rust/crates/typefacts/src/transcript.rs",
    "rust/crates/typefacts/src/bin/typefacts-transcript.rs",
  ].map((prefix) => ({
    prefix,
    owner: "the Type Facts differential harness (test tooling; not read by the checker)",
    checks: ["differential-harness"],
  })),
  // The incrementality tools, one exact path each: neighbours in scripts/ stay
  // unmapped on purpose.
  ...[
    "scripts/ir-edit-fuzz.mjs",
    "scripts/ir-edit-fuzz.test.mjs",
    "scripts/ir-read-lint.mjs",
    "scripts/ir-read-lint.test.mjs",
    "scripts/ir-read-lint.allowlist.json",
  ].map((prefix) => ({
    prefix,
    owner: "the IR incrementality tools (lint and random-edit harness)",
    checks: ["ir-tools-test", "ir-read-lint", "ir-edit-fuzz"],
  })),
  { prefix: "packages/cli/", owner: "packages/cli", checks: ["bun-test-cli"] },
  { prefix: "packages/wasm/", owner: "packages/wasm", checks: ["bun-test-wasm", "wasm-parity"] },
  // Prose that no build, test, or gate reads: the universal set only. Only
  // paths checked to be unread by code belong here. `docs/rules/` is read by
  // the CLI tests and emitted in every finding's URL, and other `docs/`
  // subtrees feed manifests and hints, so the rest of `docs/` stays unmapped.
  ...[
    "docs/adr/",
    "docs/typefacts/adr/",
    "docs/typefacts/differential.md",
    "docs/precision-backlog.md",
    "docs/decisions.md",
    "docs/open-work.md",
    "docs/README.md",
    ".claude/",
    "AGENTS.md",
    "CLAUDE.md",
    "CONTEXT.md",
    "CONTRIBUTING.md",
  ].map((prefix) => ({ prefix, owner: "prose read by no check", checks: [] })),
].sort((a, b) => b.prefix.length - a.prefix.length);

/**
 * The table row a path belongs to, or `null` when nothing claims it. A prefix
 * ending in `/` claims a directory; any other prefix claims exactly one file.
 */
export function classify(path) {
  return (
    ROWS.find((row) => (row.prefix.endsWith("/") ? path.startsWith(row.prefix) : path === row.prefix)) ??
    null
  );
}

/**
 * The plan for a change set.
 *
 * @returns {{full: boolean, unmapped: string[], decisions: {path: string, row: string, checks: string[]}[], checks: string[]}}
 */
export function planFor(paths) {
  const decisions = [];
  const unmapped = [];
  const selected = new Set();
  for (const path of [...new Set(paths)].sort()) {
    const row = classify(path);
    if (!row) {
      unmapped.push(path);
      decisions.push({ path, row: "no row matches", checks: [] });
      continue;
    }
    decisions.push({ path, row: row.owner, checks: row.checks });
    for (const check of row.checks) selected.add(check);
  }
  if (unmapped.length > 0) {
    return { full: true, unmapped, decisions, checks: ["verify"] };
  }
  // Prerequisite, not a row: a gate that runs the checker must run *this*
  // source's checker.
  const needsChecker = ["coverage", "ownership-gate", "ir-edit-fuzz"].some((check) => selected.has(check));
  const ordered = [
    // In every plan, mapped or empty: the producer is gitignored, so no change
    // set can report that it needs rebuilding. The stamp check is ~10ms when it
    // is already current.
    "build-typefacts",
    ...UNIVERSAL,
    ...(needsChecker ? ["build-debug"] : []),
    // Before the process tests: it rebuilds with the harness pins recomputed,
    // and everything after it should run against that binary.
    ...["facts-lib", "ir-lib", "backend-process", "backend-staleness", "dialects-process"].filter((id) =>
      selected.has(id)
    ),
    ...["coverage", "ownership-gate"].filter((id) => selected.has(id)),
    ...["ir-read-lint", "ir-tools-test", "ir-edit-fuzz"].filter((id) => selected.has(id)),
    // Separate workspace; its tests drive the producer built above.
    ...["typefacts"].filter((id) => selected.has(id)),
    ...["differential-harness"].filter((id) => selected.has(id)),
    // Before the WASM package's own tests: its Make target builds the package
    // (owned artifacts included) that those tests load on a fresh tree.
    ...["wasm-parity"].filter((id) => selected.has(id)),
    // The parity gate builds the package, which compiles the WASM crate
    // itself, so the cheap compile check is only for plans without it.
    ...(selected.has("wasm-compile") && !selected.has("wasm-parity") ? ["wasm-compile"] : []),
    ...["bun-test-cli", "bun-test-wasm"].filter((id) => selected.has(id)),
  ];
  return { full: false, unmapped, decisions, checks: ordered };
}

/**
 * Whether `make verify` must build the WASM package and run its parity gate
 * for a change set (ADR 0379).
 *
 * Parity compares two builds of one analysis. An edit confined to the analysis
 * crates -- the rows that select `wasm-compile` -- moves both builds alike,
 * apart from what only the WASM target does differently; for the analysis that
 * is threads, which `clippy.toml` in each of those crates confines to sites
 * with a sequential arm. Prose no check reads moves neither. Anything else, an
 * unreadable change set, or an empty one (nothing to attribute the result to)
 * runs it. CI runs parity on every pull request either way.
 *
 * @returns {{run: boolean, reason: string}}
 */
export function wasmParityScope(changed) {
  if (changed.error) return { run: true, reason: changed.error };
  if (changed.paths.length === 0) return { run: true, reason: "no change set to scope by" };
  const outside = [...new Set(changed.paths)].filter((path) => {
    const row = classify(path);
    return !row || !(row.checks.includes("wasm-compile") || row.checks.length === 0);
  });
  if (outside.length > 0) {
    return { run: true, reason: `${outside.length} changed path(s) outside the analysis crates, e.g. ${outside[0]}` };
  }
  return { run: false, reason: `all ${new Set(changed.paths).size} changed path(s) are analysis-only or prose` };
}

const git = (args) => {
  const result = spawnSync("git", args, { cwd: ROOT, encoding: "utf8" });
  if (result.status !== 0) {
    return { error: (result.stderr || result.stdout || `git ${args[0]} failed`).trim() };
  }
  return { stdout: result.stdout };
};

/**
 * Paths from `git status --porcelain -z`, including both sides of a rename.
 *
 * `-z` is not a detail. Without it git *quotes* any path outside a narrow safe
 * set, using C-style **octal** escapes -- `"fixtures/reactive-ir/caf\303\251/App.tsx"`
 * -- which are not valid JSON escapes, so `JSON.parse` throws and a single
 * non-ASCII changed path takes the whole plan down with an unhandled
 * `SyntaxError`. It also quotes inconsistently from a reader's point of view: a
 * path containing a space is *not* quoted. `-z` removes quoting entirely and
 * separates records with NUL, which cannot occur in a path.
 *
 * Record shape: `XY <path>\0`, and for a rename or copy `XY <new>\0<orig>\0` --
 * two NUL-terminated fields rather than the ` -> ` of the non-`-z` form. A
 * rename contributes both sides: the old path's row still owns whatever stopped
 * existing there.
 */
export function porcelainPaths(output) {
  const paths = [];
  const records = output.split("\0");
  for (let index = 0; index < records.length; index += 1) {
    const record = records[index];
    if (record === "") continue;
    const status = record.slice(0, 2);
    paths.push(record.slice(3));
    if (/[RC]/.test(status)) {
      index += 1;
      const origin = records[index];
      if (origin) paths.push(origin);
    }
  }
  return paths;
}

/** Where the ignored producer's local source identity is recorded. */
const PRODUCER_STAMP = "bin/solid-typefacts.buildinfo";

/**
 * Whether the built producer matches this local source manifest, as a sentence or
 * `null`.
 *
 * `bin/` is gitignored, so a producer built from other inputs is a change no
 * change set can report -- and the producer computes every fact the repository
 * reasons about. A drifted or absent stamp therefore escalates: not because a
 * narrow check is missing, but because nothing here knows which answers moved.
 *
 * The build profile is the one field compared loosely: a `bench-dev` producer
 * (thin LTO, `TYPEFACTS_PROFILE`) and a release one are the same sources
 * through the same compiler, so either answers for the other here. `make verify`
 * still certifies with release.
 */
export function producerStampDrift({ expected, stamp }) {
  if (expected === null) {
    return "cannot compute the local Type Facts source identity";
  }
  if (stamp === null) {
    return `${PRODUCER_STAMP} is absent, so the built producer's source identity is unknown`;
  }
  let recorded;
  try {
    recorded = JSON.parse(stamp);
  } catch {
    return `${PRODUCER_STAMP} does not contain a valid local source identity`;
  }
  const withoutProfile = ({ profile: _profile, ...rest } = {}) => JSON.stringify(rest);
  if (recorded === null || typeof recorded !== "object" || withoutProfile(recorded) !== withoutProfile(expected)) {
    return (
      `the built producer identity does not match the local Type Facts source manifest; ` +
      `every fact in the repository comes from it`
    );
  }
  return null;
}

/** The expected local identity and the recorded one, read off disk. */
export function readProducerIdentity(root = ROOT) {
  const stampFile = join(root, PRODUCER_STAMP);
  let expected = null;
  try {
    expected = typefactsIdentity(process.env.TYPEFACTS_BUILD_ID || "dev", root);
  } catch {
    // Fail closed below; the reason is deliberately independent of the
    // platform-specific filesystem/toolchain exception.
  }
  const stamp = existsSync(stampFile) ? readFileSync(stampFile, "utf8") : null;
  return { expected, stamp };
}

/**
 * The input classes the selection basis cannot see at all.
 *
 * Printed on every run rather than hidden in a document: the whole purpose of
 * the plan is to tell a reader when the fast loop may be trusted, and a
 * fail-closed claim with two unnamed holes in it is not that.
 */
export const BASIS_CAVEATS = [
  "gitignored build products (bin/solid-typefacts, bin/solid-checker-rust, rust/target/**) are " +
    "invisible to `git status`; `build-typefacts` runs in every plan and a drifted producer stamp " +
    "escalates, but a hand-replaced binary is not detected here.",
  "gitignored fixture inputs are invisible too -- notably a node_modules/solid-js dialect stub " +
    "added to an already-tracked fixture without its .gitignore exception. No row selects coverage " +
    "for it, and checkDialectStubs (which catches a substituted dialect) runs inside coverage. " +
    "Run `make verify`, or coverage directly, after touching fixture node_modules.",
];

/**
 * Everything this branch changed: the merge-base diff against `base`
 * (default `origin/main`) plus whatever the working tree has on top.
 *
 * A missing or unreadable base is not "nothing changed" -- it is an unknown
 * change set, so it escalates the same way an unmapped path does.
 */
export function changedPaths(base = "origin/main") {
  const diff = git(["diff", "--name-only", "-z", `${base}...HEAD`]);
  if (diff.error) {
    return { error: `cannot diff against ${base} (${diff.error})` };
  }
  const status = git(["status", "--porcelain", "-z"]);
  if (status.error) {
    return { error: `cannot read the working tree (${status.error})` };
  }
  return {
    paths: [
      ...diff.stdout.split("\0").filter(Boolean),
      ...porcelainPaths(status.stdout),
    ],
  };
}

const commandsOf = (check) => CHECKS[check].commands ?? [CHECKS[check].command];

const render = (command) => command.join(" ");

/**
 * Which lane each check runs in. Checks in one lane run one after another, in
 * plan order; lanes run side by side. A lane is a resource that may not be
 * shared: AGENTS.md allows one Cargo process per target directory, so every
 * check that runs Cargo in `rust/target` shares the `rust` lane, the check-only
 * builds share `lint` (LINT_TARGET), and the producer workspace
 * (`apps/solid-typefacts/target`) has its own. The gates and the JS suites run
 * built binaries only.
 */
export const LANES = {
  "build-typefacts": "producer",
  "fmt-check": "preflight",
  "whitespace-check": "preflight",
  "schema-json": "preflight",
  "dialect-manifests": "preflight",
  // Check-only builds in their own target directory (LINT_TARGET).
  clippy: "lint",
  "wasm-compile": "lint",
  "build-debug": "rust",
  "facts-lib": "rust",
  "ir-lib": "rust",
  "backend-process": "rust",
  "backend-staleness": "rust",
  "dialects-process": "rust",
  "differential-harness": "rust",
  typefacts: "typefacts",
  coverage: "gates",
  "ownership-gate": "gates",
  // The IR edit fuzz drives the debug checker's daemon, like the gates; the
  // read lint and the IR tool tests build nothing.
  "ir-edit-fuzz": "gates",
  "ir-read-lint": "preflight",
  "ir-tools-test": "preflight",
  // The CLI launcher falls back to `make build-rust` when it finds no
  // checker, so the suite queues with the other `rust/target` users.
  "bun-test-cli": "rust",
  // `make wasm-parity` builds in both target directories, and the package
  // tests load what it built.
  "wasm-parity": "wasm",
  "bun-test-wasm": "wasm",
  verify: "verify",
};

/**
 * The schedule for a plan: each check with its lane and the checks that must
 * pass before it starts.
 *
 * - The producer comes first: every other check reads facts it computes.
 * - The cheap preflight checks come next, so a formatting slip fails in
 *   seconds rather than after a build.
 * - The gates run the debug checker, so they wait for the `rust` lane.
 * - The WASM lane builds in both workspaces' target directories, so it waits
 *   for every other Cargo check.
 */
export function scheduleFor(checks) {
  const lane = (check) => LANES[check] ?? "verify";
  const inLanes = (...names) => checks.filter((check) => names.includes(lane(check)));
  return checks.map((check) => {
    const after = new Set();
    const own = lane(check);
    if (own !== "producer") for (const prior of inLanes("producer")) after.add(prior);
    if (!["producer", "preflight"].includes(own)) {
      for (const prior of inLanes("preflight")) after.add(prior);
    }
    // After every `rust/target` build, not only `build-debug`: a `cargo test`
    // with other dev-dependency features can relink the binary a gate runs.
    if (own === "gates") for (const prior of inLanes("rust")) after.add(prior);
    if (own === "wasm") for (const prior of inLanes("rust", "lint", "typefacts")) after.add(prior);
    if (own === "verify") for (const prior of checks) if (prior !== check) after.add(prior);
    return { check, lane: own, after: [...after] };
  });
}

/**
 * Runs a schedule: lanes side by side, each lane in order, every check only
 * after the checks it waits for passed. Output is buffered per command and
 * printed whole, so lanes never interleave. The first failure stops the run:
 * running commands are terminated and nothing new starts.
 *
 * @returns {Promise<{check: string, command: string[], status: number, error?: string} | null>}
 */
export async function runSchedule(schedule, { commandsOf, environmentOf, spawnCommand = spawn }) {
  const settled = new Map();
  const outcome = new Map();
  for (const { check } of schedule) {
    let resolveCheck;
    outcome.set(check, new Promise((resolve) => (resolveCheck = resolve)));
    settled.set(check, resolveCheck);
  }
  const running = new Set();
  let failure = null;
  const stopAll = () => {
    for (const child of running) {
      try {
        process.kill(-child.pid, "SIGTERM");
      } catch {
        // Already gone.
      }
    }
  };
  // The children have process groups of their own (so a failure can stop a
  // whole build), which also keeps a terminal's Ctrl-C from reaching them:
  // pass it on, and stop everything on any exit.
  const signals = ["SIGINT", "SIGTERM", "SIGHUP"];
  const onSignal = (signal) => {
    stopAll();
    process.exit(128 + { SIGINT: 2, SIGTERM: 15, SIGHUP: 1 }[signal]);
  };
  for (const signal of signals) process.on(signal, onSignal);
  process.on("exit", stopAll);
  const runCommand = (check, command) =>
    new Promise((resolve) => {
      const [executable, ...args] = command;
      const output = [];
      const child = spawnCommand(executable, args, {
        cwd: ROOT,
        env: environmentOf(check, command),
        detached: true,
        stdio: ["ignore", "pipe", "pipe"],
      });
      running.add(child);
      child.stdout?.on("data", (chunk) => output.push(chunk));
      child.stderr?.on("data", (chunk) => output.push(chunk));
      let spawnError = null;
      child.on("error", (error) => (spawnError = error));
      child.on("close", (code, signal) => {
        running.delete(child);
        resolve({ output: Buffer.concat(output), status: spawnError ? 127 : (code ?? (signal ? 1 : 0)), error: spawnError?.message });
      });
    });
  const lanes = new Map();
  for (const entry of schedule) {
    const previous = lanes.get(entry.lane) ?? Promise.resolve(true);
    const run = (async () => {
      const ready = (await previous) && (await Promise.all(entry.after.map((check) => outcome.get(check)))).every(Boolean);
      if (!ready || failure) return false;
      for (const command of commandsOf(entry.check)) {
        if (failure) return false;
        const started = Date.now();
        const result = await runCommand(entry.check, command);
        const seconds = ((Date.now() - started) / 1000).toFixed(2);
        process.stdout.write(`\n----- ${entry.check} [${entry.lane}]: ${render(command)}\n`);
        process.stdout.write(result.output);
        if (result.status !== 0) {
          if (!failure) {
            failure = { check: entry.check, command, status: result.status, error: result.error, seconds };
            stopAll();
          }
          return false;
        }
        console.log(`=== check ${entry.check} ${seconds}s`);
      }
      return true;
    })();
    run.then((passed) => settled.get(entry.check)(passed));
    lanes.set(entry.lane, run);
  }
  await Promise.all(outcome.values());
  for (const signal of signals) process.off(signal, onSignal);
  process.off("exit", stopAll);
  return failure;
}

/** Where scripts/verify.sh records the last clean, fully green commit. */
const GREEN_STAMP = "rust/target/verify-green-commit";

/**
 * `--base REV`, then `SOLID_VERIFY_BASE`, then the last green commit when it
 * is an ancestor of HEAD, then `origin/main`.
 */
export function baseOf(argv, env, green = null) {
  const index = argv.indexOf("--base");
  if (index !== -1) {
    const value = argv[index + 1];
    if (!value || value.startsWith("--")) return { error: "--base needs a revision" };
    return { base: value };
  }
  if (env.SOLID_VERIFY_BASE) return { base: env.SOLID_VERIFY_BASE };
  if (green) return { base: green, green: true };
  return { base: "origin/main" };
}

/** The recorded green commit, only if it is still an ancestor of HEAD. */
export function greenCommit(root = ROOT) {
  const stamp = join(root, GREEN_STAMP);
  if (!existsSync(stamp)) return null;
  const commit = readFileSync(stamp, "utf8").trim();
  if (!/^[0-9a-f]{40}$/.test(commit)) return null;
  const ancestor = spawnSync("git", ["merge-base", "--is-ancestor", commit, "HEAD"], { cwd: root });
  return ancestor.status === 0 ? commit : null;
}

/** Printed decisions are capped; a large change set is summarized by row. */
const DECISION_PRINT_LIMIT = 40;

async function main() {
  // `--wasm-scope [--base REV]`: scripts/verify.sh asks whether to run the
  // WASM package and parity gate. The scope is the whole branch against
  // origin/main (or `--base`/SOLID_VERIFY_BASE), not the last green commit:
  // it is what CI's parity run will cover.
  if (process.argv.includes("--wasm-scope")) {
    const selected = baseOf(process.argv, process.env);
    const scope = wasmParityScope(selected.error ? { error: selected.error } : changedPaths(selected.base));
    console.log(`${scope.run ? "run" : "skip"}: ${scope.reason}`);
    return 0;
  }
  const dryRun = process.argv.includes("--dry-run");
  const drift = producerStampDrift(readProducerIdentity());
  const selected = baseOf(process.argv, process.env, greenCommit());
  if (selected.error) {
    console.error(`verify-delta: ${selected.error}`);
    return 2;
  }
  const { base } = selected;
  if (selected.green) console.log(`verify-delta: base is the last fully green commit ${base.slice(0, 12)}.`);
  const changed = changedPaths(base);
  let plan;
  if (drift) {
    console.log(`verify-delta: ${drift}.`);
    console.log(
      "verify-delta: the producer is gitignored, so no change set can report what it moved." +
        " Escalating to the full `make verify` -- fail closed.",
    );
    plan = { full: true, unmapped: [], decisions: [], checks: ["verify"] };
  } else if (changed.error) {
    console.log(`verify-delta: ${changed.error}`);
    console.log("verify-delta: escalating to the full `make verify` -- fail closed.");
    plan = { full: true, unmapped: [], decisions: [], checks: ["verify"] };
  } else if (changed.paths.length === 0) {
    console.log(`verify-delta: nothing changed against ${base} or in the working tree.`);
    plan = { full: false, unmapped: [], decisions: [], checks: ["build-typefacts", ...UNIVERSAL] };
  } else {
    plan = planFor(changed.paths);
  }

  console.log(`\n=== verify-delta: mapping ${plan.decisions.length} changed path(s) against ${base} ===`);
  if (plan.decisions.length <= DECISION_PRINT_LIMIT) {
    for (const decision of plan.decisions) {
      console.log(
        `  ${decision.path}\n      row: ${decision.row}` +
          `\n      checks: ${decision.checks.length ? decision.checks.join(", ") : "(none)"}`,
      );
    }
  } else {
    const byRow = new Map();
    for (const decision of plan.decisions) byRow.set(decision.row, (byRow.get(decision.row) ?? 0) + 1);
    for (const [row, count] of byRow) console.log(`  ${count} path(s): ${row}`);
    console.log(`  (more than ${DECISION_PRINT_LIMIT} paths; pass --base to narrow the change set)`);
  }
  if (plan.full && plan.unmapped.length > 0) {
    const shown = plan.unmapped.slice(0, DECISION_PRINT_LIMIT);
    const hidden = plan.unmapped.length - shown.length;
    console.log(
      `\nverify-delta: ${plan.unmapped.length} path(s) match no row in the mapping:` +
        `\n  ${shown.join("\n  ")}` +
        (hidden > 0 ? `\n  … and ${hidden} more` : "") +
        `\nAn unmapped path can change any answer here, so the plan escalates to the full` +
        ` \`make verify\` rather than guessing. Add a row only when a directory's blast radius` +
        ` is genuinely known.`,
    );
  }
  if (!plan.full) {
    console.log(
      "\n=== verify-delta: what the selection basis cannot see ===\n" +
        BASIS_CAVEATS.map((line) => `  - ${line}`).join("\n"),
    );
  }
  console.log(`\n=== verify-delta: plan (${plan.checks.length} check(s)) ===`);
  for (const check of plan.checks) {
    const reason = CHECKS[check].why;
    console.log(`  ${check}${reason ? ` -- ${reason}` : ""}`);
    for (const command of commandsOf(check)) console.log(`      ${render(command)}`);
  }
  if (dryRun) {
    console.log("\nverify-delta: --dry-run, nothing executed.");
    return 0;
  }

  console.log("");
  // Later Bun/Make drivers can invoke Cargo too. Keep the same pins across the
  // whole selected plan.
  const plannedCargo = plan.checks.some((check) => commandsOf(check).some(([executable]) => executable === "cargo"));
  const environment = plannedCargo ? certificationEnvironment(ROOT) : process.env;
  const failure = await runSchedule(scheduleFor(plan.checks), {
    commandsOf,
    environmentOf: (check) => ({ ...environment, ...(CHECKS[check].env ?? {}) }),
  });
  if (failure) {
    console.error(
      `\n=== verify-delta FAILED at ${failure.check} after ${failure.seconds}s: ${render(failure.command)}` +
        (failure.error ? ` (${failure.error})` : ` (exit ${failure.status})`),
    );
    return failure.status || 1;
  }
  console.log(
    "\nverify-delta: every selected check passed. `make verify` remains the handoff authority.",
  );
  return 0;
}

if (process.argv[1] && resolve(process.argv[1]) === resolve(fileURLToPath(import.meta.url))) {
  process.exit(await main());
}
