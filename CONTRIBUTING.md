# Contributing

The checker and CLI are Rust, and the repository-owned Type Facts producer
(Rust, on the tsrs TypeScript checker) lives under `apps/solid-typefacts`, its
own Cargo workspace. `scripts/build-typefacts.sh` builds it from local source. Keep the fact boundary explicit: Oxc owns syntax, the Solid
compiler owns execution semantics, and tsrs owns checker facts.

## Prerequisites

- Rust 1.99.1 with `rustfmt` and `clippy`, and the `wasm32-wasip1` target (the
  Type Facts producer and its WASM reactor) and `wasm32-wasip1-threads` (the
  WASM lane of `make verify`; without it the build stops at "can't find crate
  for `core`"): `rustup target add wasm32-wasip1 wasm32-wasip1-threads`
- A one-time dependency fetch for each workspace, because the compiler-identity
  step runs `cargo metadata --offline` and fails on a fresh machine until it
  has run: `cargo +1.99.1 fetch --manifest-path rust/Cargo.toml` and
  `cargo +1.99.1 fetch --manifest-path apps/solid-typefacts/Cargo.toml`
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
lane's own steps. `verify-performance` still runs alone. When the branch
changes only the analysis crates (`solid-facts`, `solid-reactive-ir`, the v2
rules) or prose, it skips the WASM package and parity gate, which CI runs on
every pull request (ADR 0379; `SOLID_CHECKER_VERIFY_WASM=1` forces it). Rust suites run under
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

## The pre-release run and the reproduction set

Precision work iterates on `fixtures/uncertifiable-repro` (ADR 0404), a few
small projects on published rc.17 bytes: one short case per cause or shape, in
one table, `cases.json`, with a `kind` (`uncertifiable`, `violation`, `clean`).
`make repro-causes` checks that each case still produces what it stands for,
`make repro-budget` that the set stays small, and `make coverage` pins its
findings, so a slice that certifies a case moves its snapshot. Between
releases, what users see arrives as issues with minimal repros; nothing else is
taken in.

Once before each release, run the checker locally over many pinned public
Solid 2 repositories, then discard them:

1. Write the release manifest (shape of `scripts/corpus/rc17-manifest.json`:
   each app's repository and commit, its projects, the configuration), restore
   it (clone at the commits, install frozen), and build the release checker
   (`make build-checker-release`).
2. Sweep it, and once more with `--program-boundary closed` for the
   open-program counterfactual:
   `node scripts/corpus-sweep.mjs --manifest <m> --apps-root <apps> --out sweep.json`.
   For the dispatch veto's share (ADR 0405), sweep once more with a scratch
   build in its own worktree whose `withholds_owner_proofs` and
   `withholds_read_proofs` (`indexes/event_delivery.rs`) return `false`
   (`--checker <that build>`), and never commit it.
3. Uncertifiable results: group them by cause,
   `node scripts/uncertifiable-causes.mjs --report sweep.json --closed-report closed.json --noveto-report noveto.json --cases fixtures/uncertifiable-repro/cases.json`.
   A cause a case stands for updates that case's `labelWeight` and `sites`;
   a new cause of 10 distinct sites or more becomes a case, folded into an
   existing file when its mechanism is the same; one that cannot be
   reproduced faithfully (a package beyond the budget, a checkout artifact)
   or has fewer sites goes to `notReproduced` with its reason. Then verify
   (ADR 0406): `--draw 5 --draw-out samples.json` gives each case's seeded
   sample of sites; read each from source against the case's file, record
   the verdicts in the case's `verification.samples`, and split a case whose
   samples do not all read the same. Quote only verified coverage.
4. Violations, after triage: a real defect becomes a `violation` case; a false
   positive blocks the release until the checker is fixed, and its shape
   becomes a `clean` case.
5. Clean results, by sample: a certified shape confirmed by a runtime probe
   (`scripts/runtime-oracle.mjs`) or a mutation (`scripts/mutants.mjs`)
   becomes a `clean` case, so a later change cannot turn it uncertifiable or
   into a violation.

One case per cause or shape, never per finding. To write one, reduce a sample
site with `node scripts/repro-reduce.mjs` (it mirrors the app to scratch and
keeps the result while TypeScript 7.1 stays as clean as it was), then rewrite
the lines as one file of at most 40 lines (two if the cause is cross-file) in
the project of its rule; a cause that acts on the whole project (the dispatch
veto, the program boundary) gets its own. It must type-check against the set's
published typings and must not be something `tsc` reports. Record the source
(app, file:line, commit) and the expected results, run `make repro-causes`,
`make repro-budget` and `make coverage`, read every new snapshot, then
`make coverage-update`. Adding a package to the set's `node_modules` means
copying published bytes and listing them, with version and integrity, in
`packages.json`; run `make repro-registry` (network) after adding or changing
one, to check the record against the registry.

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
