# Contributing

The checker and CLI are Rust, and the repository-owned Type Facts producer
(Rust, on the tsrs TypeScript checker) lives under `apps/solid-typefacts`, its
own Cargo workspace. `scripts/build-typefacts.sh` builds it from local source. Keep the fact boundary explicit: Oxc owns syntax, the Solid
compiler owns execution semantics, and tsrs owns checker facts.

## Prerequisites

- Rust 1.99.1 with `rustfmt` and `clippy`, and the `wasm32-wasip1` target (the
  Type Facts producer and its WASM reactor)
- CMake (the producer's mimalloc allocator)
- Node.js (the `tsc` oracle and the runtime resolver run it)
- Bun 1.4.0 (published packages remain compatible with Node.js)
- `jq`

## First clone: enable the git hooks

~~~sh
make hooks
~~~

`core.hooksPath` is local config, so a repository cannot turn its own hooks on.
The only hook refuses a commit that stages a file over 5 MB (with an allowlist
for the benchmark artifacts this repository knowingly tracks); `.gitignore`
matches names, and size is the property that decides.

## Common commands

```sh
make build        # Rust CLI and the pinned TypeFacts producer
make test         # Rust workspace and CLI adapters
make coverage     # compare every fixture project against its findings snapshot
make package      # native npm package layout
```

## Verifying a change

```sh
make verify-delta BASE=<rev>   # the checks the changed paths call for, then the universal set
make verify                    # the full authority; run before proposing a change
```

`verify-delta` diffs against the last commit a full `make verify` passed on with
a clean tree (else `origin/main`; `BASE=` overrides), prints the plan it matched, and escalates to `make verify`
for any path it cannot map. `bun scripts/verify-delta.mjs --dry-run` prints the
plan without running it. The table of narrow checks per changed area is in
[AGENTS.md](AGENTS.md). Focused loop:

```sh
make verify-fast
make test-focused TEST=<existing-test-name> TEST_PACKAGE=solid-reactive-ir
```

Use an exact existing positive or negative test for the owning crate; the driver
refuses an empty selection and supplies the current Type Facts pins.

Type Facts changes must keep the producer, Rust client, schemas, fixtures and
checker consumer green in one change; producer/client protocol changes land
atomically. Solid 2 compiler execution facts come from the pinned semantic-only
Solid fork, the only compiler fork this build carries (ADR 0110).

`make verify` builds in the edit loop's `dev` profile, so after `make
verify-delta` it reuses that build. Within a step it runs independent lanes side
by side, one Cargo process per target directory: the checker's suite and builds
in `rust/target`, the producer's in `apps/solid-typefacts/target`, clippy in
`rust/target/lint` (shared with `verify-delta`), and the WASM package in
`rust/target/wasm-package`; then the gates, WASM parity and the JS suites. Each
lane's log is printed whole when it ends, and the timing summary lists every
lane's own steps. `verify-performance` still runs alone. Rust suites run under
nextest when `cargo-nextest` is installed (`SOLID_CHECKER_RUST_TEST_RUNNER=test`
forces `cargo test`; per-test durations land in
`rust/target/nextest/verify/junit.xml`); logs land in
`rust/target/verify-logs/run-*`. `make clean-verify` removes the lint and WASM
package trees.

The producer build (`scripts/build-typefacts.sh`) keeps finished binaries in
`~/.cache/solid-checker/typefacts`, keyed by the full source identity, so a new
worktree or a branch switch back reuses one instead of relinking with fat LTO
(`TYPEFACTS_SHARED_CACHE=` disables it; CI leaves it off). While iterating on
the producer, `TYPEFACTS_PROFILE=bench-dev` builds it with thin LTO;
`verify-delta` accepts either build, and `make verify` always rebuilds release.
For dependency builds shared across worktrees, install `sccache` and export
`RUSTC_WRAPPER=sccache`.

## Performance regressions

`make verify-performance` enforces repository-owned invariants over a
deterministic corpus: structural ones (export aggregation scaling, Type Facts
payload per source, cached-result reuse) and wall-time ceilings (fresh Reactive
IR per source, a one-file incremental edit on 1,000 sources). The `Performance`
workflow enforces only the structural ones, because shared runners vary too much
for a fixed ceiling; its regression gate is `benchmarks/compare-performance.mjs`,
which races the merge base against the head on one runner and fails past a 1.25x
ratio. Override the ceilings with `SOLID_CHECKER_MAX_INCREMENTAL_NS` and
`SOLID_CHECKER_MAX_FIRST_IR_NS_PER_SOURCE` to test that the gate turns red.
Pushes to `main` and pull requests also send wall-time benchmarks to CodSpeed.

## Continuous integration caches

A cache entry is worth writing only where a later run can read it. Cache scopes
are per branch: a pull request reads `main`'s caches and writes its own, which
nothing reads, and writing them anyway exhausts GitHub's 10 GB ceiling and evicts
the entries every run needs. So `Swatinem/rust-cache` restores everywhere and
saves only on `main` (`save-if`), release workflows never save, and jobs that
consume another job's dependency build name its `shared-key` with
`save-if: false`. Jobs that compile the same thing share one entry; the release
native build shares the CI release key, keyed on the runner image because
linux-x64 ships from 22.04 on purpose. The `Performance` workflow caches
benchmark binaries under the commit sha, never a measurement.

## Semantic changes

Add positive and negative fixtures, expose only the required facts, represent
the behavior in Reactive IR, add a fail-closed proof obligation, and return
evidence sufficient to explain each finding. Unsupported behavior that can
affect a proof must produce `uncertifiable`. Package-source fixtures preserve
published runtime and declaration bytes and record exact versions and integrity.

Do not infer JSX execution behavior from transformed output. Do not expose
tsrs or Oxc nodes across fact-domain interfaces.

## Releases

Maintainers publish a release by pushing a semantic-version tag such as
`v0.1.0`. The first publish needs an `NPM_TOKEN` secret in the `npm` GitHub
environment. Once all seven packages exist, configure npm trusted publishing for
each of them (this repository, `.github/workflows/publish-npm.yml`, environment
`npm`) and remove the token after verifying the first trusted release.

## Upstream code

The Solid 2 compiler remains an exact semantic-only fork dependency. Type Facts
is owned here; its external repository is import provenance, not an active
dependency or PR target. Follow [the monorepo policy](docs/monorepo.md). Oxc and
tsrs remain pinned dependencies.
