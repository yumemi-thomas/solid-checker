# destructure-store-slot-patterns

**Claim (ADR 1162).** A store destructure that setup does not provably run hands each property to SC1001's placement as
the one Get it makes (ADR 1107). This now holds whatever the property's value is, as long as its key is exact: a quoted key,
a renamed slot, a default, or an empty nested pattern (`{ a: {} }`, `{ items: [] }`). Each component is written in an effect's
apply, where the strict-read window is open:

| component | outcome |
|---|---|
| `QuotedInApply`, `EmptySubInApply`, `EmptyItemsInApply`, `EmptySiblingInApply`, `RenamedDefaultInApply`, `NamespaceQuotedInApply` | SC1003 violation (main: uncertifiable `props-callers-unenumerated`) |
| `ThenRenamedInApply`, `ThenQuotedInApply`, `ThenDefaultInApply`, `ThenNestedEmptyInApply` (a `then` Get, over a literal `then` that nothing writes) | no row (main: uncertifiable `store-key-undecided`) |
| `QuotedInHandler` | uncertifiable `function-run-unplaced`, as its member twin `store.v` there |
| `ThenWrittenBacking` (the setter is referenced), `ThenNestedInApply` (`{ then: { b } }` reads past the key), `ComputedKeyInApply` | uncertifiable |

An escaped key (`{ then: w }`) is not handed over. It is left out here because an escaped name makes the whole file
uncertifiable (SC9012). `sc1003-then-keys` covers it in a body.

Chrome: `fixtures/runtime-oracle/destructure-store-slot-patterns`. It reports STRICT_READ_UNTRACKED at the six violations
and at `ThenNestedInApply`, and nothing at the rest. Stubs: `destructure-store-placement`'s.
