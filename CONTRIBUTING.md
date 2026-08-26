# Contributing

The checker and CLI are Rust. There is no Go in this repository: the
TypeScript-Go `solid-typefacts` producer is built from its own repository by
`scripts/build-typefacts.sh`. Keep the fact boundary explicit: Oxc owns syntax,
the Solid compiler owns execution semantics, and TypeScript-Go owns checker
facts.

## Prerequisites

- Go 1.26 or newer (to build the TypeFacts producer from its pinned revision)
- Rust 1.97 with `rustfmt` and `clippy`
- Bun 1.4.0 (published packages remain compatible with Node.js)
- `jq`

## Common commands

```sh
make build       # Rust CLI and the pinned TypeFacts producer
make test        # Rust workspace and CLI adapters
make verify      # formatting, Clippy, tests, and schema validation
make package     # native npm package layout
```

Run `make verify` before proposing a change. Compiler execution semantics are
conformance-tested in the `dom-expressions` repository, not here.

Full verification keeps its Rust artifacts in `rust/target/verify` with debug
symbols and incremental object caches disabled. This bounds the disk cost of
the feature matrix without changing ordinary development and test profiles.

## Performance regressions

`make verify-performance` certifies repository-owned ceilings for scaling,
Type Facts payload, fresh Reactive IR, cached reuse, and a one-file incremental
analysis. The incremental ceiling is intentionally measured on a deterministic
1,000-source corpus and can be overridden with
`SOLID_CHECKER_MAX_INCREMENTAL_NS` when testing that the gate turns red.

The `CodSpeed` GitHub workflow sends fresh, cached-throughput, same-span
incremental, and span-shifting incremental wall-time benchmarks to CodSpeed on
`main` and every pull request. Runs on `main` establish the comparison baseline;
pull requests receive a performance check and report against that baseline.
These are wall-time benchmarks because the end-to-end analysis includes the Type
Facts child process, which CPU simulation does not follow, and they run in their
own workflow so that a red `Performance` gate still leaves a measurement on the
commit.

The cases are declared in `codspeed.yml` and executed by
`benchmarks/run-codspeed-case.mjs`. CodSpeed times the whole case process, so an
iteration count there is chosen to make its own phase dominate process start-up
and the one fresh analysis every session pays before anything can be reused.
Reproduce a case locally with the corpus the workflow generates:

```sh
bun benchmarks/generate-bench-corpus.mjs 1000 /tmp/solid-checker-codspeed-corpus
bun benchmarks/run-codspeed-case.mjs cached
```

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

The DOM Expressions compiler and the TypeFacts producer live in their own
repositories and are consumed as pinned dependencies. Follow
[the monorepo policy](docs/monorepo.md) when moving either pin. Oxc, tsgolint,
and TypeScript-Go remain pinned dependencies too.
