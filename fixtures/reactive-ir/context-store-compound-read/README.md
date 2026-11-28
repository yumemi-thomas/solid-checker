# context-store-compound-read

**Claim (ADR 1236).** A store and its setter handed in a provider's value literal (ADR 1211) and written through a `useContext` result
by a write that reads its target first (`ctx.s1.a.v += 1`, `++`, `||=`) Gets the member before it Sets it. rc.21 warns
STRICT_READ_UNTRACKED at that Get beside STORE_WRITE_OUTSIDE_SETTER at the write. The value-use census took the store write's SC2003
row for the member's (`value_use::write_decided_target`), so the Get had no row and the program was certified clean of
STRICT_READ_UNTRACKED (32 audit programs, `mx-store-write-outside-setter-*-s-context*`). It now keeps its SC9012
`value-held-in-binding` row there: uncertifiable, not a proof of the read.

| component | outcome |
|---|---|
| `CompoundChild` (`+=`), `IncrementChild` (`++`), `LogicalChild` (`||=`), `DestructuredChild` (`const { s1 } = useContext(...)`, `+=`) | SC2003 violation; SC9012 uncertifiable at the write (main: certified clean of SC1001) |
| `PlainChild` (`=`) | SC2003 violation only: a plain write Gets nothing |

Not built: SC1001's own row at that Get (a violation where Chrome proves it): the read rules do not follow a provider's value to its
readers' member reads. Chrome: `fixtures/runtime-oracle/context-store-compound-read`. Stubs: `store-write-through-prop-view`'s.
