# store-write-chain-links

**Claim (ADR 0465, ADR 0460's rule for `delete`).** A store write reads every link of its member chain past the root before it writes. Under strictNullChecks (the program's effective option, from Type Facts: here no `strict: false`, so on): a link that may be `null` or `undefined` is TS18048/TS18047, so SC2003 is silent; a link whose type excludes both keeps the violation; an element-access link or a chain under a type assertion is uncertifiable (its type is not its declared value's).

Violations (rc.17 in Chrome: `STORE_WRITE_OUTSIDE_SETTER`): `store.v = 1`, `list.push(1)` on a store that is the array, `store.user.name = "b"` and `store.items.push(1)`. Uncertifiable: `store.rows[0].done` (Chrome: dropped), and `store.maybe!.b`, `(store.opt as …).b`, which TypeScript accepts and which throw `TypeError` in Chrome (the links hold `null` and `undefined`). Silent: the optional and nullable links (`@ts-expect-error` marks TS18048/TS18047) and `store.list?.push(2)`.

`store-write-chain-links-loose` is the same shape with `strict: false`. Stub: `createStore` as loose as the published rc.17 typings (`Store<T> = T`).
