# primitive-in-effect-callback / flush-module-level

Variant of `primitive-in-effect-callback` (shares its rc.17 `node_modules`). A `flush()` statement at module level has no owner: the exemption keeps SC4005 (memo) and SC4001 (cleanup) violations.
