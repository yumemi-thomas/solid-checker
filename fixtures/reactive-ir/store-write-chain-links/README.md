# store-write-chain-links

**Claim (ADR 0465, ADR 0460's rule for `delete`).** A store write reads every link of its member chain past the root before it writes. A link that may be `null` or `undefined` is TS18048/TS18047 under strictNullChecks, so the write is TypeScript's and SC2003 is silent; any other link is still no runtime proof without strictNullChecks, which the facts do not record (an undefined link throws `TypeError` before the proxy sees a write), so the finding is uncertifiable, never a violation.

Violations (rc.17 in Chrome: `STORE_WRITE_OUTSIDE_SETTER`): `store.v = 1` and `list.push(1)` on a store that is itself the array (no link). Uncertifiable: `store.user.name`, `store.items.push(1)`, `store.rows[0].done` (Chrome: dropped, `STORE_WRITE_OUTSIDE_SETTER`), and `store.maybe!.b`, `(store.opt as …).b`, which TypeScript accepts and which throw `TypeError` in Chrome (the links hold `null` and `undefined`). Silent: the optional and nullable links (`@ts-expect-error` marks tsc's TS18048/TS18047) and `store.list?.push(2)`.

`store-write-chain-links-loose` is the same shape without `strict`: there the links' types drop `undefined` and `null`, so the nullable writes are uncertifiable (docs/open-work.md). Stub: `createStore` as loose as the published rc.17 typings (`Store<T> = T`).
