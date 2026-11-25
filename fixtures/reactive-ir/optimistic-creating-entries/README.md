# optimistic-creating-entries

**Claim (ADR 1216).** rc.21 throws OPTIMISTIC_WRITE_BEFORE_LOAD from an async optimistic source's setter while the source has
not answered (`devGuardOptimisticBeforeLoad`, `STATUS_UNINITIALIZED`). ADR 0636 proves the write on the creating pass through
one stack (`entry_on_stack_of`). A throw on one run is the claim, so one entry suffices (`some_entry_on_stack_of`): a direct,
straight-line call of an unrebound named helper written outside JSX, whatever its other uses, and a dialect primitive that runs
its callback during its call (`flush(fn)`, a memo's compute with no `lazy` option).

| component | SC2009 |
|---|---|
| `HelperAlsoHanded`, `MemoCompute`, `FlushInHelper`, `SharedByMemoAndHandler` | violation (main: uncertifiable) |
| `HandlerOnly`, `UnderBranch` (a call under a branch), `TimerOnly`, `InJsx` (a call written in JSX), `LazyMemo`, `AfterAwait`; the last helpers are also handed on, so no in-place layer decides them | uncertifiable |

Chrome: `fixtures/runtime-oracle/optimistic-creating-entries`. Stub: rc.21's function form of `createOptimistic`, `createMemo`
(its `lazy` option) and `flush(fn)`; `node_modules` from `rc21-optimistic-before-load`.
