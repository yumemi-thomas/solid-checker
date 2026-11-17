# sync-option-overridden-member

**Claim (ADR 1034).** `createEffect(async () => 1, apply, { sync: true })` is SC7002 (SYNC_NODE_RECEIVED_ASYNC) wherever it runs. Written in a function that is an object literal's member value which a later spread, a later computed key or a later property of the same key may replace, nothing proves that the function ever runs, so the row is uncertifiable `member-value-overridden`, never a violation. This holds through a method and a closure it calls, and through a namespace call in a module helper's callback written inside it.

Uncertifiable: `SpreadAfter`, `ComputedAfter`, `MethodSpreadAfter`, `NamespaceHelper`, `SettledSpreadAfter` (the fuzz shape, handed to `onSettled`); Chrome never runs the replaced original. Also `SpreadWithoutKey`, whose spread holds no `run`: Chrome runs the original and reports. This is a known miss, failing closed: no fact reads which keys a spread's value holds.

Violations, where the member is final (Chrome reports SYNC_NODE_RECEIVED_ASYNC at each): `SpreadBefore`, `ComputedBefore`, `Plain`, and `NestedLiteralLater` (the later `run` belongs to a nested literal).

Runtime: rc.21 Chrome, oracle case `fixtures/runtime-oracle/sync-option-overridden-member`.
