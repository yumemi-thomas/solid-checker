---
name: verify-handoff
description: Choose validation checks proportional to a solid-checker change and write the final handoff report. Use when a change slice is complete, before claiming a task done, when deciding which tests to run, or when a test run looks suspiciously green.
---

# Verification and handoff

Checks are chosen per change class, run once, and reported honestly. Do not
repeat a command while source, fixture, binary, and environment are unchanged.

## Arm the harness (trap)

Every fixture-driven process test under
`rust/crates/solid-facts-backend/tests` **skips silently** when
`SOLID_TYPEFACTS_BIN` is unset; only a canary test fails to flag it. A bare
`cargo test --workspace` reporting success there verifies nothing. Always:

~~~sh
SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" cargo +1.97 test --manifest-path rust/Cargo.toml ...
~~~

Rust **library** tests (`-p <crate> --lib`) need no binary and no env var.

## Fresh vs checked-in binary (trap)

`bin/solid-checker-rust` is checked in and may lag source. After Rust source
changes, run coverage/ownership with
`SOLID_CHECKER_BIN="$PWD/rust/target/debug/solid-checker-rust"` (build the
debug target once if missing). Never conclude "no finding moved" from a run
that may have used a stale binary. Do not rebuild or overwrite
`bin/solid-checker-rust` merely to test a source change. The Node CLI
launcher's override is `SOLID_CHECKER_NATIVE_BIN`, not `SOLID_CHECKER_BIN`.
Rebuild `bin/solid-typefacts` (scripts/build-typefacts.sh) only when the
TypeFacts revision, protocol, build id, or producer-dependent code changed.

## Proportional gates

While iterating: the one narrow check for the owning crate (see the table in
AGENTS.md). At handoff, add per change class:

- **Analyzer or fixture changes**: relevant process tests (armed), coverage
  comparison, and CLI tests if adapter-visible.
- **Fixture/finding changes**: coverage and ownership-gate comparisons; `--update`
  only after the non-updating diff showed the exact intentional change.
- **Contract changes**: `node scripts/check-bundled-contracts.mjs` and
  `node scripts/dialect-manifests.mjs check-composed-contracts`, plus the
  exact package contract generation checks.
- **Everything**, once, at the end (the universal set):

  ~~~sh
  cargo +1.97 fmt --manifest-path rust/Cargo.toml --all -- --check
  git diff --check
  jq empty schema/solid-reactivity.schema.json
  node scripts/dialect-manifests.mjs validate
  cargo +1.97 clippy --manifest-path rust/Cargo.toml --workspace --all-targets -- -D warnings
  ~~~

- **Release or broad architectural changes only**: `make verify` (full gate:
  fmt, clippy, per-dialect feature checks, armed workspace tests, coverage,
  ownership and TypeScript-oracle gates, performance certification, CLI tests,
  schema/manifest checks). Never
  run it as an iteration loop.

Run only one Cargo process at a time; parallel Cargo commands contend for the
build lock. If a check fails because a binary is stale, build that target once
and rerun the same check — do not fan out into unrelated full-suite commands.

## Committing (house bar)

Every commit must be individually green on the gates. Two proven traps: the
dialect seam moves as one piece (solid-dialect vocabulary, solid-reactive-ir
engine, and both rules catalogs cannot land in separate commits when the seam
changes), and snapshot updates belong in the commit whose code moved the
findings.

## Report format

State exactly:

- what changed and where;
- which focused fixtures and tests ran (with their actual results);
- which expensive checks ran or were intentionally deferred;
- any generated artifacts changed (snapshots, contracts, manifests);
- exact remaining fail-closed or uncertifiable cases.

Do not claim perfection while known approximations or fail-closed paths
remain; distinguish full from partial coverage of any upstream issue.
