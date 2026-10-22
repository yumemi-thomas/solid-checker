# Third-party provenance

`solid-checker` is MIT licensed. It contains and depends on third-party software
whose original notices remain applicable.

## Solid 2 compiler

- Upstream: https://github.com/solidjs/solid
- Upstream base: `5efaf260becb32293f2bcb4d32f8be72be6de674`
- Consumed semantic-only fork: https://github.com/yumemi-thomas/solid
- Semantic implementation revision: `c04c48779812d3d87166da3741c625748458c62f`
- Pinned distribution revision: `3ad4bbec37ae30f325a803cdb4271a71c86a2a2d`
- Fork branch: `solid-checker/compiler-facts-rc13`
- License: MIT

The `solidjs-compiler` crate under `packages/compiler` is consumed as a pinned
Cargo git dependency, built without its Node-API feature. The fork adds only
output-neutral semantic trace code, validation, and facts tests. The second
revision above changes only the trace's implementation-identity constant to
the first revision. The fork is maintained without an upstream pull request
and carries no compiler behavior fixes.

## Solid 1.x compiler (dom-expressions fork)

- Upstream: https://github.com/ryansolid/dom-expressions
- Consumed fork: https://github.com/yumemi-thomas/solid-1x-compiler
- Pinned revision: `ca3bbfae7d1e00e28ef73f9af58bdb46e248b512`
- License: MIT

The Solid 1.x dialect consumes the same `dom-expressions-compiler` crate name
from its own repository, kept at differential parity with the Babel compiler
Solid 1.x ships. It is consumed as a pinned Cargo git dependency and is not
copied into this repository.

## Oxc

- Upstream: https://github.com/oxc-project/oxc
- Version: `0.118`, resolved exactly by this repository's `rust/Cargo.lock`
- License: MIT

Oxc is consumed as published Rust crates. It is not forked or copied into this
repository.

## Type Facts

- Imported repository: https://github.com/yumemi-thomas/solid-ts-facts
- Imported revision: `92c53392388518d69ef27220729f5c061479deed`
- License: MIT

The exact external history was imported into this repository. The Go producer,
Rust process/session client, schemas, goldens, tests, ADRs, and benchmarks now
move together here; their original MIT license remains at
`apps/solid-typefacts/LICENSE`.

## tsgolint and TypeScript-Go

- tsgolint revision: `c3269c01a0c894a31330e1b4c3bd4edc6eb7694b`
- TypeScript-Go revision: `8d29e62f3585` (pseudo-version
  `v0.0.0-20260724234109-8d29e62f3585`)
- Resolution: root `go.mod`, `go.sum`, and all nine shim modules
- Licenses: MIT

The tsgolint-derived TypeScript-Go shim modules are vendored under `shims/` and
retain their MIT license at `shims/LICENSE`. TypeScript-Go itself remains a
pinned Go module dependency and is not copied into this repository.

## tsrs

- Upstream: https://github.com/oxc-project/tsrs
- Upstream revision: `467182f2402b140897e8a02d7a25699db0e79e2a` (v0.18.0)
- Consumed fork: https://github.com/yumemi-thomas/tsrs
- Pinned revision: `49f534856809ad8d5a596123f02175ba01675c8d`
- TypeScript commit it ports: `b85298b6a81f` (microsoft/TypeScript,
  `7.1.0-dev.20260929`)
- License: Apache-2.0 (tsrs is a derivative work of TypeScript, Copyright (c)
  Microsoft Corporation, also Apache-2.0; its `NOTICE` carries TypeScript's
  third-party notices and the bundled `lib.*.d.ts` files)

The Rust Type Facts producer in `apps/solid-typefacts-rs` consumes the tsrs
compiler crates (`tsrs_core`, `tsrs_ast`, `tsrs_scanner`, `tsrs_checker`,
`tsrs_compiler`, `tsrs_tsoptions`, `tsrs_vfs`) as pinned Cargo git
dependencies in that workspace only. The fork is upstream plus one marked
commit that exports two checker functions (`is_untyped_function_call` and
`get_reduced_apparent_type`, in `crates/tsrs_checker/src/solid_checker_exports.rs`);
it changes no behavior. tsrs is not copied into this repository. A producer
binary embeds the TypeScript library declaration files, so the Apache-2.0
terms and the tsrs `NOTICE` apply to a distributed producer.

## Solid Primitives

- Upstream: https://github.com/solidjs-community/solid-primitives
- Corpus revision: `46e038a1554cdac58b0a2f04cde735f010508061`
- License: MIT

Solid Primitives is fetched only by the optional corpus workflow. Its source is
not redistributed as part of this repository.

## eslint-plugin-solid

- Upstream: https://github.com/solidjs-community/eslint-plugin-solid
- Version: `0.14.5` (test corpus extracted from commit `6d3bc311`)
- License: MIT

Retained source snippets from the plugin's rule test suites are preserved in
the product-owned `fixtures/ownership-cases/cases.json`; the accompanying
`migration-ledger.json` reconciles all 465 former cases. The Rust rules in
`rust/crates/solid-reactive-ir/src/upstream_compat/` reproduce the plugin's
rule surface over this checker's own fact tables; the plugin's implementation
is not copied.

```
MIT License

Copyright (c) 2021 Josh Wilson

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## known-css-properties

- Upstream: https://github.com/known-css/known-css-properties
- Version: `0.30.0`
- License: MIT

The package's `data/all.json` property list is vendored as the
`KNOWN_CSS_PROPERTIES` table in
`rust/crates/solid-reactive-ir/src/upstream_compat/upstream_data.rs`, so the
ported `style-prop` rule judges the same names upstream judges.

```
MIT License

Copyright (c) 2017 Mavrix Technologies

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## html-tags

- Upstream: https://github.com/sindresorhus/html-tags
- Version: `3.3.1`
- License: MIT

The package's tag list is vendored as the `HTML_TAGS` table in
`rust/crates/solid-reactive-ir/src/upstream_compat/upstream_data.rs`, the same
list `is-html@2.0.0` compiles into the detection regex upstream's
`no-innerhtml` rule calls.

```
MIT License

Copyright (c) Sindre Sorhus <sindresorhus@gmail.com> (https://sindresorhus.com)

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```
