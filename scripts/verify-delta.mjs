#!/usr/bin/env node
// `make verify-delta`: AGENTS.md's "which check to run" table, mechanized.
//
// The table tells a reader which narrow check a change needs and which
// expensive ones can wait. Applied by hand it is a judgement call made under
// time pressure, which is the condition under which the wrong row gets picked.
// This runs the table instead: it reads what actually changed, prints the row
// it matched for every path, and runs exactly those checks plus the universal
// handoff set the table always appends.
//
//   node scripts/verify-delta.mjs             plan and run
//   node scripts/verify-delta.mjs --dry-run   print the plan, run nothing
//
// **`make verify` remains the handoff authority.** This is the fast loop, and
// it is only ever as good as the mapping below; a full run is what a claim of
// "verified" rests on.
//
// The mapping fails closed. A path that matches no row -- `scripts/`, the
// `Makefile`, `schema/`, `rust/Cargo.toml`, documentation, anything new --
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
//      `build-typefacts` (a stamp check that no-ops when the binary is already
//      at the pinned revision) is in *every* plan, and a stamp that does not
//      match `rust/Cargo.toml`'s pin escalates the whole plan.
//   2. Ignored fixture inputs -- notably a `node_modules/solid-js` dialect stub
//      added to an already-tracked fixture without its `.gitignore` exception.
//      `git status` cannot see it, so no row selects `coverage`, and
//      `checkDialectStubs` (the check that catches a silently substituted
//      dialect) lives inside `coverage`. This one is *not* closed; it is printed
//      as a caveat on every run, because closing it means hashing the fixture
//      trees, which is `coverage`'s own job.
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

export const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");

const TOOLCHAIN = "+1.97";
const MANIFEST = "rust/Cargo.toml";
const DEBUG_CHECKER = "rust/target/debug/solid-checker-rust";
const TYPEFACTS = "bin/solid-typefacts";

/**
 * The checks, keyed by the identifier AGENTS.md's table uses.
 *
 * Commands are the table's commands verbatim, including the toolchain pin and
 * the `SOLID_TYPEFACTS_BIN` arming that fixture-driven process tests skip
 * silently without.
 */
export const CHECKS = {
  "build-typefacts": {
    why:
      "the producer of every fact is gitignored, so `git status` cannot report it moving; " +
      "this is a stamp check that no-ops when it is already at the pinned revision",
    command: ["scripts/build-typefacts.sh"],
  },
  "build-debug": {
    why: "coverage and the ownership gate must run a fresh debug binary, not a packaged one that can lag rust/ source",
    command: ["cargo", TOOLCHAIN, "build", "--manifest-path", MANIFEST, "--workspace"],
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
  "contract-process": {
    command: [
      "cargo", TOOLCHAIN, "test", "--manifest-path", MANIFEST, "-p", "solid-facts-backend",
      "--test", "contracts_process", "--test", "dialects_process",
    ],
    env: { SOLID_TYPEFACTS_BIN: join(ROOT, TYPEFACTS) },
  },
  coverage: {
    command: ["node", "scripts/coverage.mjs"],
    env: {
      SOLID_CHECKER_BIN: join(ROOT, DEBUG_CHECKER),
      SOLID_TYPEFACTS_BIN: join(ROOT, TYPEFACTS),
    },
  },
  "ownership-gate": {
    command: ["node", "scripts/ownership-gate.mjs", "--require-retained", "--require-complete"],
    env: {
      SOLID_CHECKER_BIN: join(ROOT, DEBUG_CHECKER),
      SOLID_TYPEFACTS_BIN: join(ROOT, TYPEFACTS),
    },
  },
  conformance: {
    commands: [
      ["node", "scripts/check-bundled-contracts.mjs"],
      ["node", "scripts/check-contract-pins.mjs"],
      ["node", "scripts/generate-solid1-runtime-surface.mjs", "--check"],
      ["node", "scripts/dialect-manifests.mjs", "check-composed-contracts"],
    ],
  },
  "npm-test-cli": { command: ["npm", "test", "--prefix", "packages/cli"] },
  "npm-test-wasm": { command: ["npm", "test", "--prefix", "packages/wasm"] },
  // The universal handoff set, appended to every plan.
  "fmt-check": {
    command: ["cargo", TOOLCHAIN, "fmt", "--manifest-path", MANIFEST, "--all", "--", "--check"],
  },
  "whitespace-check": { command: ["git", "diff", "--check"] },
  "schema-json": { command: ["jq", "empty", "schema/solid-reactivity.schema.json"] },
  "dialect-manifests": { command: ["node", "scripts/dialect-manifests.mjs", "validate"] },
  clippy: {
    command: [
      "cargo", TOOLCHAIN, "clippy", "--manifest-path", MANIFEST, "--workspace", "--all-targets",
      "--", "-D", "warnings",
    ],
  },
  verify: { command: ["scripts/verify.sh"] },
};

/** AGENTS.md's universal handoff set, in the order the document lists it. */
export const UNIVERSAL = [
  "fmt-check",
  "whitespace-check",
  "schema-json",
  "dialect-manifests",
  "clippy",
];

/**
 * One row of AGENTS.md's table, as a prefix test.
 *
 * Prefixes are matched longest-first, so `rust/crates/solid-facts-backend/`
 * cannot be swallowed by `rust/crates/solid-facts`. Nothing here matches by
 * name fragment or regex: a row applies to a directory, exactly.
 */
export const ROWS = [
  { prefix: "rust/crates/solid-facts/", owner: "solid-facts (AST, normalized facts)", checks: ["facts-lib"] },
  {
    prefix: "rust/crates/solid-reactive-ir/",
    owner: "solid-reactive-ir (IR, indexes, contracts, interprocedural, rules engine)",
    checks: ["ir-lib", "coverage"],
  },
  {
    prefix: "rust/crates/solid-facts-backend/",
    owner: "solid-facts-backend process/diagnostics",
    checks: ["backend-process", "coverage"],
  },
  // No `rust/crates/solid-dialect/` row, deliberately. AGENTS.md's table has
  // none, and the crate owns the shared `Dialect` interface that
  // `solid-reactive-ir` and both dialect crates consume -- so a change there can
  // move findings, break the IR library tests, and break the backend's process
  // tests alike. Writing a row narrower than that blast radius is exactly the
  // guess this script must not make; adding a row is a policy change to the
  // table, made there first. Until then the path is unmapped and escalates.
  {
    prefix: "rust/dialects/",
    owner: "dialects, contracts at the process boundary",
    checks: ["contract-process", "conformance"],
  },
  {
    // `pkg/contracts/bundled/**` reaches the analyzer through `include_bytes!`
    // (rust/crates/solid-facts-backend/src/diagnostics.rs), so editing one
    // changes what every fixture project's findings are -- and changes nothing
    // at all until the binary is rebuilt. AGENTS.md's Known trap says so
    // explicitly: build rust/target/debug first, then run coverage and
    // ownership. A row that ran neither would be the trap, mechanized.
    prefix: "pkg/contracts/",
    owner: "dialects, contracts at the process boundary (compiled into the binary via include_bytes!)",
    checks: ["contract-process", "conformance", "coverage", "ownership-gate"],
  },
  {
    prefix: "fixtures/",
    owner: "fixtures or expected findings",
    checks: ["coverage", "ownership-gate"],
  },
  { prefix: "packages/cli/", owner: "packages/cli", checks: ["npm-test-cli"] },
  { prefix: "packages/wasm/", owner: "packages/wasm", checks: ["npm-test-wasm"] },
].sort((a, b) => b.prefix.length - a.prefix.length);

/** The table row a path belongs to, or `null` when nothing claims it. */
export function classify(path) {
  return ROWS.find((row) => path.startsWith(row.prefix)) ?? null;
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
  const needsChecker = ["coverage", "ownership-gate"].some((check) => selected.has(check));
  const ordered = [
    // In every plan, mapped or empty: the producer is gitignored, so no change
    // set can report that it needs rebuilding. The stamp check is ~10ms when it
    // is already current.
    "build-typefacts",
    ...(needsChecker ? ["build-debug"] : []),
    ...["facts-lib", "ir-lib", "backend-process", "contract-process"].filter((id) => selected.has(id)),
    ...["coverage", "ownership-gate", "conformance"].filter((id) => selected.has(id)),
    ...["npm-test-cli", "npm-test-wasm"].filter((id) => selected.has(id)),
    ...UNIVERSAL,
  ];
  return { full: false, unmapped, decisions, checks: ordered };
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

/** Where the producer's identity is recorded, and where its pin is declared. */
const PRODUCER_STAMP = "bin/solid-typefacts.buildinfo";
const CARGO_MANIFEST = "rust/Cargo.toml";

/**
 * Whether the built producer is the one this source pins, as a sentence or
 * `null`.
 *
 * `bin/` is gitignored, so a producer at some other revision is a change no
 * change set can report -- and the producer computes every fact the repository
 * reasons about. A drifted or absent stamp therefore escalates: not because a
 * narrow check is missing, but because nothing here knows which answers moved.
 */
export function producerStampDrift({ pinned, stamp }) {
  if (pinned === null) {
    return `cannot read the typefacts revision pinned in ${CARGO_MANIFEST}`;
  }
  if (stamp === null) {
    return `${PRODUCER_STAMP} is absent, so the built producer's revision is unknown (pin ${pinned})`;
  }
  const recorded = /revision=([0-9a-f]{40})/.exec(stamp)?.[1] ?? null;
  if (recorded === null) {
    return `${PRODUCER_STAMP} records no revision (pin ${pinned})`;
  }
  if (recorded !== pinned) {
    return (
      `the built producer is at ${recorded} but ${CARGO_MANIFEST} pins ${pinned}; ` +
      `every fact in the repository comes from it`
    );
  }
  return null;
}

/** The pinned revision and the recorded one, read off disk. */
export function readProducerIdentity(root = ROOT) {
  const manifest = join(root, CARGO_MANIFEST);
  const stampFile = join(root, PRODUCER_STAMP);
  const pinned = existsSync(manifest)
    ? (/^typefacts = .*\brev = "([0-9a-f]{40})"/m.exec(readFileSync(manifest, "utf8"))?.[1] ?? null)
    : null;
  const stamp = existsSync(stampFile) ? readFileSync(stampFile, "utf8") : null;
  return { pinned, stamp };
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
 * Everything this branch changed: the merge-base diff against `origin/main`
 * plus whatever the working tree has on top.
 *
 * A missing or unreadable `origin/main` is not "nothing changed" -- it is an
 * unknown change set, so it escalates the same way an unmapped path does.
 */
export function changedPaths() {
  const diff = git(["diff", "--name-only", "-z", "origin/main...HEAD"]);
  if (diff.error) {
    return { error: `cannot diff against origin/main (${diff.error})` };
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

function main() {
  const dryRun = process.argv.includes("--dry-run");
  const drift = producerStampDrift(readProducerIdentity());
  const changed = changedPaths();
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
    console.log("verify-delta: nothing changed against origin/main or in the working tree.");
    plan = { full: false, unmapped: [], decisions: [], checks: ["build-typefacts", ...UNIVERSAL] };
  } else {
    plan = planFor(changed.paths);
  }

  console.log(`\n=== verify-delta: mapping ${plan.decisions.length} changed path(s) ===`);
  for (const decision of plan.decisions) {
    console.log(
      `  ${decision.path}\n      row: ${decision.row}` +
        `\n      checks: ${decision.checks.length ? decision.checks.join(", ") : "(none)"}`,
    );
  }
  if (plan.full && plan.unmapped.length > 0) {
    console.log(
      `\nverify-delta: ${plan.unmapped.length} path(s) match no row in AGENTS.md's table:` +
        `\n  ${plan.unmapped.join("\n  ")}` +
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
  for (const check of plan.checks) {
    for (const command of commandsOf(check)) {
      const started = Date.now();
      const [executable, ...args] = command;
      const result = spawnSync(executable, args, {
        cwd: ROOT,
        stdio: "inherit",
        env: { ...process.env, ...(CHECKS[check].env ?? {}) },
      });
      const seconds = ((Date.now() - started) / 1000).toFixed(2);
      if (result.error || result.status !== 0) {
        console.error(
          `\n=== verify-delta FAILED at ${check} after ${seconds}s: ${render(command)}` +
            (result.error ? ` (${result.error.message})` : ` (exit ${result.status})`),
        );
        return result.status || 1;
      }
      console.log(`=== check ${check} ${seconds}s`);
    }
  }
  console.log(
    "\nverify-delta: every selected check passed. `make verify` remains the handoff authority.",
  );
  return 0;
}

if (process.argv[1] && resolve(process.argv[1]) === resolve(fileURLToPath(import.meta.url))) {
  process.exit(main());
}
