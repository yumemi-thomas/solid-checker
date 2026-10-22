# primitive-in-effect-callback / render-module-level

Variant of `primitive-in-effect-callback` (shares its rc.17 `node_modules`). `render(() => App(), root)` as a module statement (the usual entry) does not withhold: SC4005 violation inside App.
