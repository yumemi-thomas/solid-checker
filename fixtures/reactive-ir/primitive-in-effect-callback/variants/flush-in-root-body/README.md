# primitive-in-effect-callback / flush-in-root-body

Variant of `primitive-in-effect-callback` (shares its rc.17 `node_modules`). A `flush()` in a `createRoot` body drains the queued apply under the root (`runEffect` never resets the context), so the memo in the apply is created owned: SC4005 is uncertifiable, not a violation (ADR 0362). The `onCleanup` in the same apply is SC4001 uncertifiable too (the root's region does not answer for an apply a flush may drain).
