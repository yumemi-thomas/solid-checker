# primitive-identity-scope / package-effect

Variant of `primitive-identity-scope` (shares its `node_modules`). The fake package's own `createEffect(source, apply)` calls `apply` synchronously on its caller's stack, here a `createRoot` body. It is not Solid's `createEffect` (ADR 0365), so the `createMemo` and `onCleanup` in its `apply` are owned by the root: silent. Before ADR 0365 the package's declaration was named Solid's `createEffect` by its path, and the `onCleanup` was an SC4001 obligation in an effect apply that does not exist.
