# store-copy-element-write

**Claim (ADR 1104, d2n).** A write through an element of a copy a store array's method answers (`const c = st.items.slice();
c[0].v = 5`) is a write to the store's element, which the store drops: an SC2003 row stands at the write, uncertifiable (the flow
cannot prove the element is the store's on every route). Before, Type Facts placed `slice` nowhere (no standard declaration), the
container flow followed nothing through it, and the only row was the store-method obligation at the `slice()` call (ADR 0638), which
stays.

| handler | rows |
|---|---|
| `slice` | the `slice()` call (ADR 0638) and the write `c[0].v` |
| `filter` | the write (as before) |
| `alias` (`const c = st.items`) | the write (as before) |
| `plain` (twin: a plain array's `slice`) | none |

`solid-js.d.ts` and the `node_modules` stub are `store-sub-value-flow`'s.
