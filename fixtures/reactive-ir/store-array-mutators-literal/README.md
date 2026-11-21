# store-array-mutators-literal

**Claim (ADR 1170).** `sort`, `reverse`, `fill` and `copyWithin` read the length and may write nothing (ADR 0718). On a store
path whose links ADR 0491 proves still hold the store's own literal (a function-local `createStore` of a closed literal, the
setter never referenced, no proxy leaving), the array literal decides it:
- `reverse()` and `sort()` write on two or more elements. `sort` takes only number literals, with no comparator or
  `(a, b) => b - a`, so it cannot throw before it writes back.
- `fill` and `copyWithin` write on a non-empty range of integer literal bounds.

Each element write meets the read proxy's `set` trap outside the setter (rc.21 `dist/dev.js:6663-6668`).

| component | outcome |
|---|---|
| `SortNumbers`, `SortComparator`, `ReverseTwo`, `FillOne`, `FillRange`, `CopyWithinTwo`, `NamespaceReverse` | SC2003 violation (main: uncertifiable `store-operation-may-not-write`) |
| `ReverseOne`, `SortOne`, `FillEmptyRange`, `CopyWithinPastEnd` (no write: Chrome silent) | uncertifiable, as before |
| `SortStrings`, `SortOtherComparator`, `SetterReferenced` | uncertifiable, as before |

Chrome: `fixtures/runtime-oracle/store-array-mutators-literal`. Stubs: `store-mutators`'s.
