# rc9-heldout-execution-shapes

Five held-out-sweep shapes do not run in the component body, so they are not proven strict reads or owned-scope writes: a primitive inline callback (`flush`) in a callback prop, `latest()` in a helper running after an `await`, a default-parameter initializer, a function source of `merge()` (rc.9 wraps it in `createMemo`), a write in a listener handed to an unknown subscriber inside a memo compute; `latest()` and helper defaults called in the body stay proven. Declarations copy `@solidjs/signals@2.0.0-rc.9` `dist/types` verbatim.

`DefaultParameterCalledInBody` is uncertifiable since ADR 0339 round five: a default that is more than a literal runs before the body and may throw, so a call of the arrow-bound `read` is not proven to reach it (`draft()` could throw first). Accepted trade, in docs/open-work.md.

Since the flip of ADR 0967's default (the review of #216) this fixture's program has an open boundary (no `.solid-checker/runtime.json`), so each `flush()` written in an exported function keeps an uncertifiable row of every code its program censuses ruled out (SC2006 `flush-in-action-unaccounted`, SC2008 `flush-in-effect-unaccounted`, SC3001 `leaf-operation-unaccounted`): an export may be entered by code this build cannot see, an action's step among it. The snapshot's additional rows are those; the claim above is unchanged.
