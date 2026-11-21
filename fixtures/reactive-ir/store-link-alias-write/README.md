# store-link-alias-write

**Claim (ADR 1169 part 2, R3 G15).** An exact `const` alias of a store link (`const held = s1.a`, an immutable identifier binding
whose whole initializer is a static chain from the store's own binding) holds that link's proxy when ADR 0491's proof holds for the
store and its literal holds an object literal at that key. Through the alias:
- a write is dropped outside the setter (SC2003 violation; the alias's uses must be contained uses, `link_alias_contained`);
- a Get is the store's Get, an SC1001 violation (a synthetic member read at the alias's member, `exact_link_alias_chain`).

| component | outcome |
|---|---|
| `IifeWrite`, `TimerWrite`, `DeepWrite` (plus its Get `held.b`), `AliasRead` | violations (main: SC2003 uncertifiable `unproven-container-root`; the Gets had no row before part 1) |
| `SetterReferenced`, `HandedOn`, `LetAlias`, `ArrayAlias` | uncertifiable, as before |

Chrome: `fixtures/runtime-oracle/store-link-alias-write`. Stub: `store-write-through-prop`'s.
