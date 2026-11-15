# rc21-optimistic-setter-forms

**Claim (ADR 0987, slice 8a).** SC2009 (`optimistic-write-before-load`) asks one question wherever an optimistic setter is recorded, never answering by absence from `async_sources`: an optimistic creation (`createOptimistic`, `createOptimisticStore`) whose first argument is not a data value, and that declares no loading value, is async, whichever form hands the setter out.

| form | async source | synchronous twin |
|---|---|---|
| declaration `const [l, set] = createOptimistic(…)` | `Declared` | `SyncDeclared` |
| assignment `[l, set] = createOptimistic(…)` | `Assigned` | `SyncAssigned` |
| a root's callback result `createRoot(() => createOptimistic(…))` | `ViaRoot` | `SyncViaRoot` |
| a project factory's result | `ViaFactory` | `SyncViaFactory` |

Before the slice the assignment and callback-result forms recorded the setter and its primitive without the decision, so `set(2)` raised nothing: a false clean, which the runtime oracle case `fixtures/runtime-oracle/optimistic-setter-forms` shows in Chrome (OPTIMISTIC_WRITE_BEFORE_LOAD at the `set(2)` of `Assigned` and `ViaRoot`).
