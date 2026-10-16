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

`core.hooksPath` is local config, so a repository cannot turn its own hooks
on. The only hook refuses a commit that stages a file over 5 MB, with an
allowlist for the benchmark artifacts this repository knowingly tracks. It
exists because bulk measurement transcripts have twice been large enough to
matter and once reached shared history by accident; `.gitignore` matches
names, and size is the property that actually decides.

## Common commands

```sh
make build       # Rust CLI and the pinned TypeFacts producer
make test        # Rust workspace and CLI adapters
make verify      # formatting, Clippy, tests, and schema validation
make package     # native npm package layout
```

Run `make verify` before proposing a change. Type Facts changes must keep the
Go producer, Rust client, schemas, fixtures, and checker consumer green in one
change. Solid 2 compiler execution facts come from the pinned semantic-only
Solid fork. It is the only compiler fork this build carries: the Solid 1.x
dialect, and its separate fork, were retired in 2026-09 (ADR 0110).

Full verification keeps its Rust artifacts in `rust/target/verify` with debug
symbols and incremental object caches disabled. This bounds the disk cost of
the feature matrix without changing ordinary development and test profiles.

Verification pins the Type Facts producer and source manifest before every
Cargo build. Producer/client protocol changes must land atomically.

For a scheduling comparison, run
`SOLID_CHECKER_RUST_TEST_RUNNER=nextest make verify`. The checked-in
`scripts/nextest.toml` uses eight test workers and writes individual durations
to `rust/target/nextest/verify/junit.xml`. The nextest path explicitly runs
doctests afterward; the built-in runner remains available through
`SOLID_CHECKER_RUST_TEST_RUNNER=test`.
With optimized hashing, one backend-suite comparison measured Cargo at 43.42s
and nextest at 41.01s (426 tests passed in each). That small difference does
not justify changing the default runner; use nextest when per-test timings
help identify the next bottleneck.

Verification runs the Go race suite and Rust workspace suite concurrently,
after preflight and oracle provisioning. There is still only one Cargo process;
both suites must pass before verification advances, and all test processes stop
before performance measurements. Each run writes separate logs and a status/timing
summary to `rust/target/verify-logs/run-*`. Interruption stops the test process
groups, including their descendants. Defaults are `GOMAXPROCS=4` and
`RUST_TEST_THREADS=8`; explicit environment values take precedence.
Use `SOLID_CHECKER_VERIFY_PARALLEL=0 make verify` for sequential execution on
constrained machines or to diagnose contention. An uncached comparison on the
development host completed both suites in 50.04s (Go 50.04s, Rust 46.96s), versus
roughly 90–110s sequentially. Go's normal test cache remains enabled; that
comparison explicitly used `go test -race -count=1` to avoid cached timings.

## Focused tooling loop

```sh
make verify-fast
make test-focused TEST=<existing-test-name> TEST_PACKAGE=solid-reactive-ir
```

Use an exact existing positive or negative test for the owning crate. The
focused driver refuses an empty selection and supplies the current Type Facts
pins. Coverage uses a fresh checker; `make verify` is the final authority.
Package-source fixtures preserve published runtime and declaration bytes and
record exact versions and integrity. Unavailable external behavior stays
uncertifiable.

## Performance regressions

`make verify-performance` certifies repository-owned invariants over a
deterministic corpus: structural ones (export aggregation scaling, Type Facts
payload per source, cached-result reuse) and wall-time ceilings (fresh Reactive
IR per source, a one-file incremental edit on 1,000 sources). Run locally, both
kinds are enforced. The `Performance` workflow enforces only the structural
ones and reports the wall-time ceilings (`--wall-time-gate report`): GitHub's
shared runners measure the same commit up to 1.6x apart between runs, so a
fixed ceiling there is either too loose to catch anything or fails on a slow
day, as it did in three of ten runs in September 2026. The workflow's
regression gate is `benchmarks/compare-performance.mjs`, which races the
merge base against the head on the same runner in interleaved rounds and fails
past a 1.25x ratio; its ratios stayed within 0.98-1.04 over the same runs. The
ceilings can be overridden with `SOLID_CHECKER_MAX_INCREMENTAL_NS` and
`SOLID_CHECKER_MAX_FIRST_IR_NS_PER_SOURCE` when testing that the gate turns red.

The `Performance` GitHub workflow also sends fresh, cached-throughput, and
one-file incremental wall-time benchmarks to CodSpeed on `main` and every pull
request. Runs on `main` establish the comparison baseline; pull requests receive
a performance check and report against that baseline. These are wall-time
benchmarks because the end-to-end analysis includes the Type Facts child
process, which CPU simulation does not follow.

## Continuous integration caches

Two rules keep the workflows from paying twice for the same build, and both are
easy to break by copying an existing step.

A cache entry is only worth writing where some later run can read it. Cache
scopes are per branch: a pull request reads the caches on `main` and writes its
own, which nothing on `main` will ever read, and a tag's scope is unreadable
even by the next tag. Writing them anyway took this repository past GitHub's
10GB ceiling, at which point the entries every run *does* read were evicted and
each build started cold. So `Swatinem/rust-cache` restores everywhere and saves
only on `main` (`save-if`), release workflows restore and never save, and the
jobs that merely consume another job's dependency build — the corpus workflows —
name that job's `shared-key` with `save-if: false`.

Where two jobs compile the same thing, they should share one entry rather than
keep two. The release native build shares the CI release key, keyed on the
runner image rather than the platform, because linux-x64 ships from 22.04 on
purpose and a dependency's C objects from a newer image have no business in that
artifact. The `Performance` workflow caches a whole benchmark runtime under its
commit sha, which is what stops it from building the base binaries a previous
run already built; it caches binaries only, never a measurement.

## Semantic changes

Add positive and negative fixtures, expose only the required facts, represent
the behavior in Reactive IR, add a fail-closed proof obligation, and return
evidence sufficient to explain each finding. Unsupported behavior that can
affect a proof must produce `uncertifiable`.

Do not infer JSX execution behavior from transformed output. Do not expose
TypeScript-Go or Oxc nodes across fact-domain interfaces.

## Releases

Maintainers publish a release by pushing a semantic-version tag such as
`v0.1.0`. For the first publish, add an `NPM_TOKEN` secret to the `npm` GitHub
environment. After all seven packages exist, configure npm trusted publishing
for each of them for this repository and `.github/workflows/publish-npm.yml`;
set the trusted environment to `npm` and allow `npm publish`. Subsequent
releases use OIDC and do not need the token; remove the `NPM_TOKEN` environment
secret after verifying the first trusted release.

## Upstream code

The Solid 2 compiler remains an exact semantic-only fork dependency; the
separate Solid 1.x compiler fork went with that dialect in 2026-09 (ADR 0110).
Type Facts is owned here; its external
repository is import provenance, not an active dependency or PR target. Follow
[the monorepo policy](docs/monorepo.md). Oxc and TypeScript-Go remain pinned
dependencies.
