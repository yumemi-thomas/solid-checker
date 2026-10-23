# primitive-in-effect-callback / flush-unresolved-any

Variant of `primitive-in-effect-callback` (shares its rc.17 `node_modules`). A call spelled `flush` on a `declare const flush: any`, in a function: SC4005 and SC4001 (the cleanup in the apply) uncertifiable. The module-level effect is still an SC4001 violation.
