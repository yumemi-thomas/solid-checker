# destructure-store-slots

**Claim (ADR 1107).** A store destructure's SC1003 row takes the verdict SC1001 gives its slots read as members (`const { v } = s` is `s.v`). Following the review of ADR 1106:

| component | outcome |
|---|---|
| `HandedAndCalled` (a function handed by name to `createMemo` and also called by the body) | SC1003 violation (ADR 1166: the body's call runs the destructure; it was uncertifiable, and silent before ADR 1107) |
| `SlotWithDefault` (`{ v = 1 }`: the slot is still read), `RestElement` (the rest reads the whole object) | SC1003 violation |
| `EmptyPattern` (`const {} = store` reads nothing) | no row |
| `ComputedKey` (a key no fact names: a symbol or `then` would not warn) | SC1003 uncertifiable `store-key-undecided` |

Placement cases are in `destructure-store-placement`. Chrome: `fixtures/runtime-oracle/destructure-store-slots`. Stubs: `destructure-store-placement`'s.
