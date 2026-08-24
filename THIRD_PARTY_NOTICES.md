# Third-party provenance

`solid-checker` is MIT licensed. It contains and depends on third-party software
whose original notices remain applicable.

## DOM Expressions

- Upstream: https://github.com/ryansolid/dom-expressions
- Consumed fork: https://github.com/yumemi-thomas/dom-expressions
- Pinned revision: `c6008f01df199ff0f0d072093e2393ed3d67f0c4`
- License: MIT

The `dom-expressions-compiler` crate is consumed as a pinned Cargo git
dependency, built without its Node-API feature. It is not forked or copied into
this repository; the semantic trace that `solid-checker` reads is maintained
upstream in that fork.

## Solid 1.x compiler (dom-expressions fork)

- Upstream: https://github.com/ryansolid/dom-expressions
- Consumed fork: https://github.com/yumemi-thomas/solid-1x-compiler
- Pinned revision: `b66c3e34ba2a0b74238726eb2b83f767eacf94f2`
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

## TypeFacts

- Consumed repository: https://github.com/yumemi-thomas/solid-ts-facts
- Pinned revision: `e2f7ac5ce2784f9e4f5bc53f4e100040f6fce3d4`
- License: MIT

The Rust client and Go producer move together at this revision. The producer
is built from the same pin and validates its protocol, schema, and build
identity when the checker starts.

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
