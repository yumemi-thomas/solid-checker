# Claude instructions for solid-checker

AGENTS.md is canonical. Follow it fully; this file is the short operating
profile for Claude-style sessions, especially the rules that prevent repeated
builds and speculative debugging.

## Minimal-tooling protocol

1. Start with one combined read-only pass:

   ~~~sh
   git status --short && rg -n "symbol-or-rule|nearby-fixture" rust packages scripts fixtures docs
   ~~~

2. Read the owning implementation, one nearby test, and one nearby fixture.
   State the semantic claim and the smallest expected finding change before
   editing.
3. Patch with `apply_patch`. Preserve all unrelated dirty and untracked work.
4. Run exactly one narrow check for that claim. Do not launch parallel Cargo
   commands; they contend for the build lock.
5. Only after the slice is coherent, run coverage once, then the handoff set.

Use Rust library tests for Rust library changes; they need no native binary.
Use process tests, coverage, parity, and contract generation only when the
process boundary or fixture behavior is in scope. After Rust source changes,
use the fresh debug checker explicitly:

~~~sh
SOLID_CHECKER_BIN="$PWD/rust/target/debug/solid-checker-rust" \
SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" node scripts/coverage.mjs
~~~

The Node CLI override is `SOLID_CHECKER_NATIVE_BIN`; `SOLID_CHECKER_BIN` is
for the scripts. Do not rebuild the checked-in binary just because source
changed. Build once only when the chosen command actually needs a missing or
stale binary.

Do not add temporary logging and repeatedly rebuild to explore a behavior. Use
a focused fixture or existing diagnostic first. If one isolated probe is
unavoidable, add it, run one reproducer, remove it immediately with
`apply_patch`, and then run the narrow test again. Do not retry network/package
downloads during local debugging; record exact external-artifact blockers.

Never run snapshot `--update` before reviewing the non-updating diff. Never
run `make verify` during iteration.

## Soundness invariants

- Unresolved, ambiguous, or missing semantic facts fail closed or become
  uncertifiable; they never prove a violation.
- Use exact symbols, declarations, call targets, and package contracts.
  Never use names, regexes, wildcards, receiver spelling, or computed-member
  guesses as semantic evidence.
- Keep Solid 1.x and Solid 2.0 vocabulary and compiler behavior dialect-owned.
  Keep contract `schemaVersion` at 1.
- Every changed semantic branch needs focused positive/negative fixtures and
  an update to `docs/precision-backlog.md` when precision status changes.
- Do not add legacy routeData, JSX sorting, `.at()` preference,
  negative-index rules, or a monolithic reactivity rule.

## Handoff

Report changed files, focused tests, expensive checks run or deferred,
generated artifacts, and exact remaining fail-closed/uncertifiable cases.
For broad changes run once:

~~~sh
cargo +1.97 fmt --manifest-path rust/Cargo.toml --all -- --check
git diff --check
jq empty schema/solid-reactivity.schema.json
node scripts/dialect-manifests.mjs validate
cargo +1.97 clippy --manifest-path rust/Cargo.toml --workspace --all-targets -- -D warnings
~~~

Do not claim perfection while known approximations or fail-closed paths
remain.
