# primitive-in-effect-callback / flush-in-apply-only

Variant of `primitive-in-effect-callback` (shares its rc.17 `node_modules`). `flush()` only inside the apply (SC2008) runs with no owner: SC4005 stays a violation, beside SC2008.
