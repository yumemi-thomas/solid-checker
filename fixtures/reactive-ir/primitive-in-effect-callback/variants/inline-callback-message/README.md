# primitive-in-effect-callback / inline-callback-message

Variant of `primitive-in-effect-callback` (shares its rc.17 `node_modules`). A memo in a `forEach` callback of the apply is SC4005 and its message says inline callback, not "directly". The effect in a `forEach` callback gets no finding from either rule (known miss, docs/open-work.md).
