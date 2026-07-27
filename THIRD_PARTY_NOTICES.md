# Third-party provenance

`solid-checker` is MIT licensed. It contains and depends on third-party software
whose original notices remain applicable.

## DOM Expressions

- Upstream: https://github.com/ryansolid/dom-expressions
- Consumed fork: https://github.com/yumemi-thomas/dom-expressions
- Pinned revision: `209e1bf78f5616885d473dd04a68913cd6bb2ce0`
- License: MIT

The `dom-expressions-compiler` crate is consumed as a pinned Cargo git
dependency, built without its Node-API feature. It is not forked or copied into
this repository; the semantic trace that `solid-checker` reads is maintained
upstream in that fork.

## Oxc

- Upstream: https://github.com/oxc-project/oxc
- Version: `0.118`, resolved exactly by the compiler's `Cargo.lock`
- License: MIT

Oxc is consumed as published Rust crates. It is not forked or copied into this
repository.

## tsgolint and TypeScript-Go

- tsgolint revision: `c3269c01a0c894a31330e1b4c3bd4edc6eb7694b`
- TypeScript-Go revision: `2bd066d87f5b`
- Resolution: pinned Go module versions in the `solid-ts-facts` repository,
  which builds the TypeFacts producer
- Licenses: MIT

Only tsgolint's TypeScript-Go shim modules are consumed. Neither repository is
forked or copied into this repository.

## Solid Primitives

- Upstream: https://github.com/solidjs-community/solid-primitives
- Corpus revision: `46e038a1554cdac58b0a2f04cde735f010508061`
- License: MIT

Solid Primitives is fetched only by the optional corpus workflow. Its source is
not redistributed as part of this repository.
