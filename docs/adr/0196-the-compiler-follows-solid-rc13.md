# ADR 0196: The compiler follows Solid rc.13

- Status: accepted and implemented (2026-10-05). Owner approval of
  2026-10-05 to publish the fork branch.
- Owners: the compiler pin (`rust/Cargo.toml`), the adapter's provenance
  literals (`rust/dialects/solid-v2/compiler/src/lib.rs`), the identity
  document (`docs/package-contract-v2/phase4/compiler-identity.json`), the
  conformance record and the third-party notice.
- Relation: completes the rc.13 move of ADR 0194, § Consequences. The
  candidate is the one prepared and validated on 2026-10-03
  (`docs/package-contract-v2/phase22/2026-10-03-rc13-compiler-facts-rebase.md`).

## Context

The checker compiled with the semantic-facts fork at distribution `9f9a84b2`,
an rc.3-era compiler (upstream `a10cf1a1`). Since `@solidjs/vite-plugin`
next.34, consumers compile with the native `@solidjs/compiler`, and rc.13's
build is upstream `5efaf260` (the rc.13 review, compiler § 3).

A rebase of the fork onto `5efaf260` existed only locally, preserved as a Git
bundle. Cargo cannot pin a local branch durably, so adoption had waited on
publishing it.

## Decision

1. **Publish the candidate.** Distribution `3ad4bbec` is pushed as
   `yumemi-thomas/solid:solid-checker/compiler-facts-rc13`, after checking
   that it descends from upstream `5efaf260` and that every changed file is
   under `packages/compiler` (the fork's semantic-facts-only rule). The branch
   name follows the fork's existing `solid-checker/compiler-facts-v2`/`-v3`.
2. **Move every identity together.**
   - The Cargo pin moves to `3ad4bbec`.
   - The adapter's upstream, implementation (`c04c4877`) and distribution
     literals move, with the compiler-facts identity
     `solid-v2:trace3:c04c4877…` and the source-manifest digest
     `sha256:35f4874f…`, which match the rebase report.
   - The identity document, the conformance record and the notice name the
     published branch.
   - The trace version stays 3, and the compiler-facts protocol stays 2.

## Consequences

- The checker now lowers JSX as the rc.13 compiler does. Two helpers the rc.13
  compiler emits (`readShallow`, `ssrElementAttribute`) are rc.13 runtime
  exports (the rc.13 review, compiler § 4).
- No fixture finding moved, so no rule change follows from the new lowering
  on the fixture corpus.
- The rebase report's limits still apply: Universal/Dynamic and authored TSRX
  trace requests refuse; the compiler options the normalized request exposes
  do not yet include rc.13's new hoisting/source-name options; native artifact
  validation covers darwin-arm64.

## Evidence

- `make compiler-facts-identity`: upstream `5efaf260`, implementation
  `c04c4877` and distribution `3ad4bbec` verified.
- `cargo` resolved the pin from the published branch. `rust/Cargo.lock` moves
  by that one source.
- Coverage: 167 fixture projects, 883 findings, unchanged.
- `make verify` passes (879 s), after one test that pinned the old identity
  literal (`compiler_certification_child_binds_the_live_pid_request_and_materialized_output`)
  moved with the pin.
