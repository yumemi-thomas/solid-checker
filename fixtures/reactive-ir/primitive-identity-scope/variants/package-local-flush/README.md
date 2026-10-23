# primitive-identity-scope / package-local-flush

Variant of `primitive-identity-scope` (shares its `node_modules`). The fake package calls a local `const flush = whenIdle(...)` inside `preloadWhenIdle`, the shape of `@solidjs/router@2.0.0-next.41`'s `dist/preload.js:139-147`, where the corpus trace found it named Solid's `flush` by its path. It is the package's own binding (ADR 0365) and its callee resolves exactly, so it is no ADR 0362 flush site: the `createMemo` in the `createEffect` apply stays an SC4005 violation.
