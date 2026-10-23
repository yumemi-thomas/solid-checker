# primitive-identity-scope / solid-flush

Variant of `primitive-identity-scope` (shares its `node_modules`). The base fixture with `flush` imported from `solid-js`: Solid's `flush` called in a `createRoot` body drains the queued apply under the root, so SC4005 is uncertifiable (ADR 0362). The base fixture's package-local `flush` must not do the same (ADR 0365).
