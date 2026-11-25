# optimistic-creating-carriers

**Claim (ADR 1216 slice 3).** `carrier_provably_runs` (ADR 0563) takes `Set`/`Map` `forEach` on a collection built there
from an array literal of at least one entry (for a `Map`, entries written as array literals). SC2009's existence walk
also climbs a function stored once in a fresh local collection (`stored_run_sites`): one straight-line `Map.set`,
`WeakMap.set` or `Set.add` on a `const` local made by the standard constructor with no argument, whose other uses are a
standard `forEach` with a literal callback of at most one parameter, or `WeakMap.get`; the run site is a later
straight-line `forEach` whose callback calls its parameter, or a call of `get` with the stored `const` key.

| component | SC2009 |
|---|---|
| `SeededMap`, `SeededSet`, `StoredInMap`, `AddedToSetUnderUntrack`, `StoredInWeakMap` | violation (slice 2: uncertifiable) |
| `EmptySet`, `StoredThenDeleted`, `StoredNeverRun`, `RunBeforeStore`, `OtherKey` | uncertifiable |

Chrome: `fixtures/runtime-oracle/optimistic-creating-carriers`. Stub: `optimistic-creating-entries`' with `untrack`.
