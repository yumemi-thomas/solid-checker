# Agent instructions for solid-checker

This file is the repository-wide operating guide for coding agents. Read it
before changing code. CONTRIBUTING.md, rust/ARCHITECTURE.md, and the relevant
documents under docs/ remain the detailed sources of truth.

## Repository shape

solid-checker is a Rust analysis engine with TypeScript/Node adapters. The
analysis pipeline deliberately separates these semantic owners:

- rust/crates/solid-facts owns syntax and normalized fact data.
- rust/crates/solid-facts-backend owns orchestration, diagnostics, and the
  process/session boundary.
- rust/crates/solid-reactive-ir owns project indexes, interprocedural analysis,
  contracts, reachability, and proof obligations.
- rust/crates/solid-dialect owns the shared dialect interface.
- rust/dialects/solid-v1 owns Solid 1.x vocabulary, compiler integration, and
  rules.
- rust/dialects/solid-v2 owns Solid 2.0 vocabulary, compiler integration, and
  rules.
- packages/cli owns the Node CLI, ESLint adapter, package metadata, and tests.
- packages/wasm owns the WASM adapter.
- scripts/ owns fixture coverage, upstream parity, contract generation, and
  packaging workflows.
- fixtures/ contains focused semantic fixtures and expected findings.
- schema/ and pkg/contracts/bundled/ contain versioned public contract artifacts.

Keep these seams explicit. Do not move TypeScript-Go or Oxc nodes across fact
interfaces, do not put dialect-specific behavior in shared code when the
dialect seam can express it, and do not turn the analyzer into one monolithic
reactivity rule.

## Safety and dirty-worktree rules

The worktree may already contain substantial user changes. Before editing:

1. Run git status --short and inspect relevant diffs.
2. Preserve unrelated modifications and untracked files.
3. Use apply_patch for source, fixture, documentation, and configuration edits.
4. Never use git reset --hard, git checkout --, git clean, or broad
   deletion/overwrite commands unless the user explicitly requests that exact
   operation.
5. Keep generated changes scoped to the fixture or artifact being tested. Do
   not rewrite all snapshots merely to make a check pass.

If an existing change overlaps the requested edit, understand it first and make
the smallest compatible patch. Do not “clean up” unrelated code while working
on a semantic issue.

## Precision contract

This project certifies behavior; it is not a syntax-pattern collection.

- Report a violation only when semantic facts and the execution model prove it.
- When a required fact, symbol, call target, package contract, or compiler
  behavior is missing, fail closed or produce an explicit uncertifiable result.
- Never treat “unresolved” or “not found” as proof of undefined, unsafe, or
  non-reactive behavior.
- Resolve exact symbols and declarations. Do not use the smallest contained
  symbol, name-only matching, regex trust, wildcard trust, or guessed member
  dispatch as a substitute for semantic resolution.
- Preserve legitimate transparent TypeScript wrappers when classifying spans or
  callees, while rejecting ambiguous computed/member targets conservatively.
- External behavior is contract-driven. Unknown packages and callback helpers
  remain uncertifiable unless semantic, type, or exact package-contract facts
  establish the behavior.
- Keep contract schemaVersion at 1; add only backward-compatible fields and
  update validation/tests when doing so.
- Preserve exact Solid 1.x and Solid 2.0 behavior. Do not infer an API from its
  name alone or share vocabulary between dialects without an explicit dialect
  owner.
- Do not implement legacy SolidStart routeData, JSX sorting, .at() preference,
  negative-index style rules, or another unrelated lint rule.

Every semantic branch should have focused positive and negative regression
fixtures. If a branch changes expected findings, update only its snapshot and
record the precision/backlog status in docs/precision-backlog.md.

## Fast implementation loop

Do not start with make verify for every edit. It intentionally includes the
slowest repository-wide work: full Clippy, workspace tests, coverage, parity,
performance, package installation, and contract validation.

### Tooling budget

Prefer one inspection pass, one focused patch, and one focused check per
semantic slice. Do not repeat a command while the source, fixture, binary, and
environment are unchanged. Before launching a command, identify which fact it
will establish; if it establishes nothing new, skip it.

- Combine the initial status and search into one command. Read only the
  owning implementation, its nearest test, and the relevant fixture before
  editing.
- Run only one Cargo build/test/clippy process at a time. Parallel Cargo
  commands contend for the same build lock and make progress slower.
- Rust library tests do not require a native binary. Process tests, fixture
  coverage, parity, and contract generation do.
- After Rust source changes, use the fresh debug binary explicitly for local
  integration checks:

  ~~~sh
  SOLID_CHECKER_BIN="$PWD/rust/target/debug/solid-checker-rust" \
    SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" node scripts/coverage.mjs
  ~~~

  The CLI launcher uses `SOLID_CHECKER_NATIVE_BIN`, not
  `SOLID_CHECKER_BIN`. Do not rebuild or overwrite `bin/solid-checker-rust`
  merely to test a source change; build it only when a checked-in/package
  binary is specifically required.
- Do not add temporary debug prints to a hot path as a first diagnostic. Use a
  focused fixture or existing test output first. If instrumentation is truly
  necessary, make it one isolated patch, run one reproducer, remove it with
  apply_patch immediately, and then run the focused check again.
- Do not use network/package installation to investigate a local semantic
  failure. For an external package, use an isolated temporary directory and
  the exact audited artifact/dependency contracts; if the artifact or network
  is unavailable, record the blocker instead of retrying or weakening proofs.
- Do not run coverage or parity with `--update` until the non-updating command
  has shown the exact intentional finding change. Never use snapshot updates
  to discover what the implementation does.

The normal cadence is: focused test after a semantic slice, one coverage or
parity comparison after fixture work, then one handoff verification pass. If a
check fails because the binary is stale, build the required target once and
rerun that same check; do not fan out into unrelated full-suite commands.

First locate the owning layer and an existing nearby fixture:

~~~sh
rg -n "relevant-symbol-or-rule" rust packages scripts fixtures docs
git status --short
~~~

Use the narrowest useful check while iterating:

~~~sh
# AST/fact changes
cargo +1.97 test --manifest-path rust/Cargo.toml -p solid-facts --lib

# Reactive IR, indexes, contracts, or interprocedural changes
cargo +1.97 test --manifest-path rust/Cargo.toml -p solid-reactive-ir --lib

# Backend process/diagnostic fixture changes
SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" cargo +1.97 test --manifest-path rust/Cargo.toml -p solid-facts-backend --test diagnostics_process

# Contract or dialect process fixtures
SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" cargo +1.97 test --manifest-path rust/Cargo.toml -p solid-facts-backend --test contracts_process --test dialects_process

# CLI and WASM adapters
npm test --prefix packages/cli
npm test --prefix packages/wasm
~~~

Reuse the checked-in bin/solid-typefacts when it is present. Rebuild it with
scripts/build-typefacts.sh only when the TypeFacts revision, protocol, build
id, or producer-dependent code changed. make build is useful when the native
binary is stale, but it is not a required precondition for every Rust unit
test.

For fixture coverage, compare first. When Rust source changed, point at the
fresh debug binary so a stale ignored binary cannot hide the change:

~~~sh
SOLID_CHECKER_BIN="$PWD/rust/target/debug/solid-checker-rust" \
SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" node scripts/coverage.mjs
~~~

Use node scripts/coverage.mjs --update only after verifying that the changed
finding set is intentional. The same rule applies to scripts/parity.mjs and
its --update mode. Snapshot updates are evidence of a deliberate semantic
change, not a repair for an unexplained diff.

During iteration, prefer targeted Clippy such as:

~~~sh
cargo +1.97 clippy --manifest-path rust/Cargo.toml -p solid-reactive-ir --lib -- -D warnings
~~~

Run workspace-wide Clippy once at handoff. Use SOLID_CHECKER_TIMINGS=1 when
diagnosing a genuinely slow analysis rather than guessing which phase is slow.

## Required validation before handoff

Choose checks proportional to the change, then run the full relevant set before
claiming completion:

~~~sh
cargo +1.97 fmt --manifest-path rust/Cargo.toml --all -- --check
git diff --check
jq empty schema/solid-reactivity.schema.json
node scripts/dialect-manifests.mjs validate
cargo +1.97 clippy --manifest-path rust/Cargo.toml --workspace --all-targets -- -D warnings
~~~

For analyzer or fixture changes, also run the relevant Rust process tests with
SOLID_TYPEFACTS_BIN, coverage comparison, and CLI/TypeScript tests. For
contract changes, run bundled-contract validation and the exact package
contract generation checks. For release or broad architectural changes, use
make verify and report any environment-dependent checks separately.

Do not repeatedly run the full suite after every small patch. A good cadence is:
targeted test after each semantic slice, coverage comparison after fixture work,
then one full verification pass.

## Fixtures, diagnostics, and snapshots

Fixtures should isolate one semantic claim. Include the source, declaration
stubs, tsconfig.json, and package stubs needed to make resolution explicit.
For a new semantic path, include at least:

- a positive case that must be diagnosed or certified;
- a negative case that must remain clean;
- an unresolved, shadowed, generic, namespace, member, or wrapper case when
  that distinction is part of the behavior;
- an assertion that the result is a proven violation versus an uncertifiable
  result, where applicable.

Prefer exact symbol identity and resolved declaration locations in tests. Add
cross-file cases whenever the implementation claims project-level resolution.
For Solid primitives, test both dialect vocabulary and namespace imports when
those paths differ.

## Contracts and dependency pins

Contracts describe exact package/version behavior at a trust boundary. Keep
generated contracts tied to the package artifact and the audited dialect
version. Test unknown external behavior as fail-closed; do not add blanket
trust to make a fixture green.

When testing a real external package, use an isolated temporary directory and
record the exact version. Do not modify checked-in node_modules or bundled
contracts accidentally. The repository audits Solid 1.x and a specific Solid
2.0 prerelease; a newer prerelease must be reviewed rather than silently
substituted.

Upstream compiler and TypeFacts dependencies are pinned by revision. If a pin
must move, update the corresponding notice and follow docs/monorepo.md; do not
vendor or float a branch.

## Documentation and final report

Update the nearest rule page, architecture note, or docs/precision-backlog.md
when behavior or precision status changes. Do not claim an upstream issue is
fully resolved if only one path is covered; distinguish full and partial
coverage and name remaining approximations.

The final report should state:

- what changed and where;
- which focused fixtures and tests ran;
- which expensive checks ran or were intentionally deferred;
- any generated artifacts changed;
- exact remaining fail-closed or uncertifiable cases.
