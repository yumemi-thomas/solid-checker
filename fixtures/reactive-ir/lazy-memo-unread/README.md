# lazy-memo-unread

**Claim (ADR 0576).** rc.17 `computed(fn, options)` runs `!options?.lazy && recompute(self, true)`: a `createMemo(fn, { lazy: true })` (or a function-form `createSignal`/`createOptimistic` with that option) computes at the first read of its accessor, not at its creation. A site that runs only in a lazy computation is proven to run only where a direct call of its `const` accessor may run; otherwise a violation there is uncertifiable (ADR 0458's class: a violation in code that never runs is a false positive).

Obligations (uncertifiable): `UnreadWrite` (SC2001), `UnreadContext` (SC4002), `UnreadAction` (SC2002), `UnreadHelper` (a helper only the compute calls), `UnreadNamedCompute` (the compute handed by name), `UnreadNamespace`, `UnreadSignalForm`, `UnknownLazy` (`lazy: flag`), `HandedAccessor` (`keep(m)`: a read not proven).

Violations: `NotLazy`, `LazyFalse`, `ReadInBody` (a render-expression read), `ReadInHandler` (a lazy memo read in a handler computes there, still an owned scope: rc.17 reports the write), `ReadSignalForm`.

Premises: the closed program is the `private` manifest. Runtime: fuzzer cells `lg-recall` `memo-lazy-unread` (16 of 16 unexecuted, 3 Chrome-confirmed) and `mx-recall` `memo-lazy-read-handler` (reported).
