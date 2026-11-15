# resolve and until in a callback their primitive runs as a computation

ADR 0997. `SlotRun::Deferred` is the attribution answer (whose reads a callback subscribes); it does not say that no observer is active where the callback
runs. `Dialect::deferred_slot_observer` states that apart, and the `slot-runs-elsewhere` silence of SC2004 and SC2005 stands only on a stated "none".

Cells (`...R` is `resolve`, `...U` is `until`):

- obligations, `computation-error-unobserved`: a call in the callback of `resolve` or in the predicate of `until` (the import, a const alias, the namespace
  member). `@solidjs/signals@2.0.0-rc.21` hands the callback to `watch(fn, …)`: an observer is active, the guard throws (Chrome rc.21), the throw rejects the
  promise the holder returned, and no report of that rejection is proven, so the row is no violation;
- obligations, no observer proven either way: a call in `from`'s subscribe function (Chrome rc.21 throws when a memo consumes the iterator);
- silences: `static_api.resolve_scope.slot-runs-elsewhere` for an effect's apply and a cleanup of a memo that re-runs, `slot-clears-observer` for a call in an
  `untrack` written inside a `resolve` callback (Chrome rc.21: quiet for each);
- no site: a local function named `resolve`.

Chrome: `fixtures/runtime-oracle/resolve-own-computation` and `resolve-in-from-subscribe`.
