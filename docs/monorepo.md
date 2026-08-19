# Monorepo and upstream policy

This repository is the build and review home for the `solid-checker` analysis
path: the Rust checker, its schemas, tests, and corpus automation. The JSX
compiler and the TypeFacts producer are pinned dependencies maintained in their
own repositories.

## Module seams

Physical colocation does not merge the module interfaces:

- `rust/crates/solid-facts-backend` orchestrates certification.
- `rust/crates/solid-facts` owns the fact model, including compiler-fact
  integration behind the `CompilerFactsProvider` seam.
- `rust/crates/solid-dialect` owns the Solid vocabulary seam both language
  versions answer through.
- `rust/dialects/solid-v2` owns Solid 2.0 specifics: the rule catalog and the
  dom-expressions compiler adapter.
- `rust/dialects/solid-v1` owns Solid 1.x specifics: its rule catalog and the
  adapter over the `solid-1x-compiler` fork.
- The `typefacts` crate and its `solid-typefacts` producer own TypeScript-Go
  facts, in their own repository.
- Each `dom-expressions-compiler` fork owns its JSX execution and
  compiler-established ownership semantics.
- `rust/crates/solid-facts-backend` owns package contracts.

Oxc and compiler facts stay in-process.

## Fork policy

The DOM Expressions compiler is consumed as a pinned Cargo git dependency on
[a fork](https://github.com/yumemi-thomas/dom-expressions), not vendored. Its
own repository owns compiler development and conformance against the reference
transform.

To adopt new compiler work, move the `rev` of `dom-expressions-compiler` in
`rust/Cargo.toml`, record the new revision in `THIRD_PARTY_NOTICES.md`, and run
`make verify`. Pin by revision rather than branch so a build is reproducible
and every upstream move is an explicit, reviewable commit here.

The Solid 1.x compiler follows the same policy on its own fork,
[solid-1x-compiler](https://github.com/yumemi-thomas/solid-1x-compiler) — the
same crate name from a second repository, kept at differential parity with
the Babel compiler Solid 1.x ships. Its `rev` moves in `rust/Cargo.toml` and
`THIRD_PARTY_NOTICES.md` exactly like the dom-expressions pin; upstream's
2.0-only codegen changes are deliberately not adopted there.

The TypeFacts producer and its Rust client are pinned the same way, on
[solid-ts-facts](https://github.com/yumemi-thomas/solid-ts-facts). They must
move together: the startup handshake compares protocol version, schema digest,
and build id, so `scripts/build-typefacts.sh` reads the revision straight out
of `rust/Cargo.toml` rather than keeping a second copy of it.

That pin is also what makes the producer cacheable. The binary is a function of
the pinned revision and the build id and nothing else, so the script records
that pair in `<output>.buildinfo` and returns immediately when it already
matches — which is why several `make` targets can each depend on
`build-typefacts` without paying for it more than once. CI keys a cache entry on
the same pair (`.github/actions/typefacts`), so a job restores the producer
instead of cloning the repository and running a cold Go build. Set
`TYPEFACTS_REBUILD=1` to force the rebuild anyway.

A release deliberately gets none of that: its build id is the tag, so every
published producer is compiled from the pinned revision during that run. The
same applies when the pin moves — a new revision is a new cache key.

Oxc, tsgolint, and TypeScript-Go stay pinned dependencies too. Prefer a pinned
dependency on a fork's own repository over vendoring sources into this tree.

## Corpus policy

Solid Primitives is a pinned compatibility corpus, not shipped code. Run
`make corpus` to clone the reviewed revision into a temporary directory, build
it, generate contracts to a fixed point, and validate all published artifacts.
Set `SOLID_PRIMITIVES_CORPUS` to reuse an existing clean checkout.
