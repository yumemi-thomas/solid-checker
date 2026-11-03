# owner-carrier-provably-runs

**Claim (ADR 0563, tightening ADR 0561).** A function handed to a reviewed standard-library carrier gets the direct call's owner edge only where the carrier provably runs it at least once (`carrier_runs::carrier_provably_runs`): `.call`/`.apply`, `Reflect.apply`, the `Promise` executor, a tagged template, an array-literal receiver with an element and no spread or hole, `reduce` with two elements or one and an initial value, `sort` with two elements that cannot be `undefined`, `Array.from` with such a literal source. A carrier that may run it zero times gives the possible-unowned edge alone: what the callback does is an obligation, never a violation.

Violations: `missing-context-provider` (SC4002) in `ReduceTwo`, `ReduceInitial`, `GlobalReflect` (`globalThis.Reflect` resolves to the standard declaration), `AssertedLiteral` (`([1] as number[])`), `PromiseExecutor`; `missing-owner` (SC4001) in `TimerNonEmpty`.

Obligations (`missing-owner` uncertifiable, never SC4002): `EmptyNamed`, `EmptyLiteral`, `PropsReceiver`, `VariableReceiver`, `SpreadReceiver`, `SetForEach`, `ReduceOne`, `SortOne`, `SortUndefined`, `FromEmpty`, `LocalReflect` (a local object, no carrier), `TimerEmpty`.

Runtime: rc.17 runs none of the obligation callbacks (an empty or unread receiver), so it throws nothing there; it throws ContextNotFoundError at each SC4002 line and warns NO_OWNER_CLEANUP in `TimerNonEmpty`. Premises: the closed program is the `private` manifest. `tsc --noEmit` clean.
