# throws-first-deferred-apply

**Claim (ADR 1220, ADR 1205 round 3).** ADR 1205's entries arm of `competing_checks::runs_only_in_applies` took a creation in a
function every entry of which is in an effect apply's callback as SC7002 `creation-throws-first`. An apply that a `flush()` runs
(during render, or from a handler when the apply has no first run: deferred, a reaction) meets no effect-callback guard first,
and Chrome reaches the result probe. The arm now answers SC4001's question only; SC7002 keeps main's answer, where an SC6002
violation proves the guard.

| component | SC7002 |
|---|---|
| `DeferredApplyClosure` (the apply deferred, rerun by a `flush()` in a handler's root) | violation (1205: uncertifiable `creation-throws-first`; main: violation) |
| `FlushBodyClosure` (a `flush()` in the body runs the apply first) | violation (1205: uncertifiable; main: violation) |
| `ApplyClosure` (no options, no flush) | uncertifiable `creation-throws-first` (as on main) |

Chrome: `fixtures/runtime-oracle/throws-first-deferred-apply`. The case is
`mx-sync-node-received-async-effect-apply-defer-flush-rerun-memo-direct-closure`, Chrome-corroborated on main.
