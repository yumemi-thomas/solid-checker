# Monorepo and upstream policy

This repository is the build and review home for the complete `solid-checker`
analysis path. The approved target co-locates the Type Facts producer and client
with the checker, while Solid compiler source follows its upstream owner through
a small semantic-only fork.

The Solid 2 compiler bootstrap and Type Facts source repatriation are complete.
The compiler remains an exact semantic-only fork dependency; Type Facts now
builds locally as one producer/client module. The external Type Facts repository
is retained only as import provenance until the retirement gate completes.

## Target module seams

Physical colocation does not merge module interfaces:

- `rust/crates/solid-facts-backend` orchestrates certification.
- `rust/crates/solid-facts` owns syntax and normalized fact-domain integration.
- `rust/crates/typefacts` owns the Rust Type Facts process/session interface.
- `apps/solid-typefacts` and its private packages own TypeScript-Go facts.
- `rust/crates/solid-dialect` owns the Solid vocabulary seam both language
  versions answer through.
- `rust/dialects/solid-v2` owns Solid 2 vocabulary and the adapter over the
  pinned `solidjs-compiler` fork.
- `rust/dialects/solid-v1` owns Solid 1.x vocabulary and the adapter over the
  separate `solid-1x-compiler` fork.
- `rust/crates/solid-facts-backend` owns package-contract acquisition and
  normalization.
- `rust/crates/solid-reactive-ir` composes facts and owns proof obligations.

Oxc and compiler facts stay in-process. Type Facts stays behind its versioned
process/session interface. Neither compiler AST nodes nor TypeScript-Go objects
cross these seams.

## Solid 2 compiler policy

Solid's Oxc compiler moved to
[`solidjs/solid/packages/compiler`](https://github.com/solidjs/solid/tree/next/packages/compiler).
The checker consumes package `solidjs-compiler` from an exact revision of
[`yumemi-thomas/solid`](https://github.com/yumemi-thomas/solid), based on a
recorded `solidjs/solid#next` commit. It will no longer develop Solid 2 facts in
the former DOM Expressions repository.

The fork's patch queue is semantic-only. Allowed patches add semantic trace
models, output-neutral recorder calls at existing decisions, trace validation
and serialization, host-independent fact access, and fact-specific tests.
Forbidden patches change lowering, generated JavaScript, source maps,
diagnostics, runtime behavior, compiler features, performance, unrelated
dependencies, or unrelated compiler implementation. A dependency addition is
allowed only when it is required by the semantic-fact interface and is isolated
from normal compiler behavior.

If fact instrumentation exposes a compiler defect, report or fix it upstream in
a separate branch and pull request. Until upstream `next` contains the behavior,
the checker fact remains open. Rebase the semantic branch after the upstream
change; never carry the compiler fix in the checker semantic branch.

Every fork revision is immutable in `rust/Cargo.toml`. A revision move records
the upstream base and semantic patch head in `THIRD_PARTY_NOTICES.md`, updates
the trace-version assertion and cache identity, and runs:

- trace-on versus trace-off output identity;
- fork versus exact-upstream output identity when facts are ignored;
- semantic-trace reconciliation and mutation tests;
- Solid 2 compiler-adapter and process fixtures;
- checker finding parity, coverage, ownership, and full verification.

The detailed transition is in
[Compiler and Type Facts bootstrap](package-contract-v2/compiler-and-typefacts-bootstrap.md).

## Solid 1.x compiler policy

Solid 1.x remains on
[`solid-1x-compiler`](https://github.com/yumemi-thomas/solid-1x-compiler), kept
at differential parity with the Babel compiler Solid 1.x ships. Solid 2
compiler relocation does not authorize importing next-only lowering into this
fork. Its exact `rev` and notice move together.

## Type Facts colocation policy

The external
[`solid-ts-facts`](https://github.com/yumemi-thomas/solid-ts-facts) history was
imported at `92c53392388518d69ef27220729f5c061479deed`. Its active layout is:

```text
go.mod / go.sum
apps/solid-typefacts/
shims/
rust/crates/typefacts/
schema/typefacts-*.json
docs/typefacts/
benchmarks/typefacts/
```

The source move is behavior-neutral. Before a new demand or protocol change:

- replay the same request transcripts through external and imported builds;
- compare encoded and decoded semantic results;
- run every Go, Rust, retained-session, cancellation, restart, stale-generation,
  memory, and performance test;
- prove checker findings are unchanged;
- keep the local path dependency and producer build atomic.

`scripts/build-typefacts.sh` builds local source. Its cache
stamp is a manifest digest over the producer, client, shims, schemas, dependency
pins, relevant toolchain identity, and build id. The startup handshake compares
protocol version, schema digest, and build identity. Codec limits are validated
from the local language-neutral schema and bound by the source-manifest stamp;
adding a separate wire-level codec digest is a deferred protocol change.

All later Type Facts changes land atomically with the Rust client, checker
consumer, proof fixtures, and corpus measurements they affect. After two clean
CI runs and one release build, the external repository becomes read-only or is
archived with a pointer to the imported commit; it is no longer an active source
or pull-request target.

## Transition invariant

The completed migration must not regress into mixed ownership states:

- external Type Facts producer with a local client;
- local producer with a git-pinned client;
- Solid compiler lowering with DOM Expressions trace fallback;
- semantic trace versions accepted by assertion from the producer's own
  constant;
- a branch name where an exact compiler revision is required.

Each transition is atomic and fails closed on a mismatch.

## Corpus policy

Solid Primitives is a pinned compatibility corpus, not shipped code. Run
`make corpus` to clone the reviewed revision into a temporary directory, build
it, generate contracts to a fixed point, and validate all published artifacts.
Set `SOLID_PRIMITIVES_CORPUS` to reuse an existing clean checkout.
