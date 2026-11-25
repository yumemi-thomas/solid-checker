# throws-first-deferred-apply

**Claim (ADR 1220).** ADR 1205's arm of `competing_checks::runs_only_in_applies` takes a creation in a function every entry of
which is in an effect apply's callback as SC7002 `creation-throws-first`: the effect-callback guard throws
PRIMITIVE_IN_EFFECT_CALLBACK on the apply's first run, before the result probe. An apply written with an options argument may
have no first run (`defer: true`), so for SC7002 such an apply does not count (`apply_written_last`).

| component | SC7002 |
|---|---|
| `DeferredApplyClosure` (the apply deferred, rerun by a `flush()` in a handler's root) | violation (1205: uncertifiable `creation-throws-first`; main: violation) |
| `ApplyClosure` (no options) | uncertifiable `creation-throws-first` |

Chrome: `fixtures/runtime-oracle/throws-first-deferred-apply`. The case is
`mx-sync-node-received-async-effect-apply-defer-flush-rerun-memo-direct-closure`, Chrome-corroborated on main.
