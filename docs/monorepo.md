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
- `rust/dialects/solid-v2` owns Solid 2.0 specifics: the rule catalog and the
  dom-expressions compiler adapter.
- The `typefacts` crate and its `solid-typefacts` producer own TypeScript-Go
  facts, in their own repository.
- The `dom-expressions-compiler` crate owns JSX execution semantics.
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

The TypeFacts producer and its Rust client are pinned the same way, on
[solid-ts-facts](https://github.com/yumemi-thomas/solid-ts-facts). They must
move together: the startup handshake compares protocol version, schema digest,
and build id, so `scripts/build-typefacts.sh` reads the revision straight out
of `rust/Cargo.toml` rather than keeping a second copy of it.

Oxc, tsgolint, and TypeScript-Go stay pinned dependencies too. Prefer a pinned
dependency on a fork's own repository over vendoring sources into this tree.

## Corpus policy

Solid Primitives is a pinned compatibility corpus, not shipped code. Run
`make corpus` to clone the reviewed revision into a temporary directory, build
it, generate contracts to a fixed point, and validate all published artifacts.
Set `SOLID_PRIMITIVES_CORPUS` to reuse an existing clean checkout.
