# Contributing

The checker and CLI are Rust, and the repository-owned TypeScript-Go producer
lives under `apps/solid-typefacts`. `scripts/build-typefacts.sh` builds it from
local source. Keep the fact boundary explicit: Oxc owns syntax, the Solid
compiler owns execution semantics, and TypeScript-Go owns checker facts.

## Prerequisites

- Go 1.26 or newer (to build and test the local Type Facts producer)
- Rust 1.97 with `rustfmt` and `clippy`
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

`verify-delta` diffs against `origin/main` unless `BASE=` names the commit the
slice started from, prints the plan it matched, and escalates to `make verify`
for any path it cannot map. `bun scripts/verify-delta.mjs --dry-run` prints the
plan without running it. The table of narrow checks per changed area is in
[AGENTS.md](AGENTS.md). Focused loop:

```sh
make verify-fast
make test-focused TEST=<existing-test-name> TEST_PACKAGE=solid-reactive-ir
```

Use an exact existing positive or negative test for the owning crate; the driver
refuses an empty selection and supplies the current Type Facts pins.

Type Facts changes must keep the Go producer, Rust client, schemas, fixtures and
checker consumer green in one change; producer/client protocol changes land
atomically. Solid 2 compiler execution facts come from the pinned semantic-only
Solid fork, the only compiler fork this build carries (ADR 0110).

`make verify` keeps its Rust artifacts in `rust/target/verify` (no debug
symbols, no incremental cache) and runs the Go race suite and the Rust suite
concurrently with one Cargo process. Logs and a timing summary land in
`rust/target/verify-logs/run-*`. `SOLID_CHECKER_VERIFY_PARALLEL=0` runs the two
sequentially on a constrained machine; `SOLID_CHECKER_RUST_TEST_RUNNER=nextest`
swaps the Rust runner and writes per-test durations to
`rust/target/nextest/verify/junit.xml`.

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
TypeScript-Go or Oxc nodes across fact-domain interfaces.

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
TypeScript-Go remain pinned dependencies.
