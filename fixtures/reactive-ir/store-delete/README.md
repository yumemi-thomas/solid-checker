# SC2003: `delete` of a store path

`delete store.path` through a `createStore` proxy (or a `createProjection`
result) is dropped outside the setter: `deleteProperty` warns
`STORE_WRITE_OUTSIDE_SETTER` (rc.17 `dist/dev.js:5761-5767`). Reported as the
assignment `store.path = x` is, with its exemptions (ADR 0461). The exports
here have no caller: their store Gets are SC1001 uncertifiable (ADR 0934).

TypeScript reports `delete` of a required (TS2790) or readonly (TS2704)
property, so the operand must carry the Type Fact `writableOptionalProperty`.
A chain past the root (`store.a.b`) gets the assignment's verdict (ADR 0465):
silent when a link may be nullish (TS18047/TS18048), a violation when every
link excludes both under strictNullChecks, uncertifiable otherwise or without
a fact.

Reported: an optional property deleted in the body, an effect's apply, a
wrapper, a parenthesized operand, another store's setter, a namespace import
(`Namespace.tsx`), a projection, a `const` alias (ADR 0463), and nested paths
(`store.a.b` in the body and a handler, `store.o.b` under `if (store.o)`).

Silent: the store's own setter, its draft, a plain object, a shadowing
parameter. Uncertifiable since ADR 0829 (Chrome warns, `tsc` accepts; the
operand is proven neither optional and writable nor rejected): a non-optional property whose type
includes `undefined` and a computed key here, and in `Unproven.tsx` a literal
computed key, an open record key, an index-signature member, an array index, a
nested computed key, `unknown` and a literal-typed variable key. Its
`RejectedKeys` (TS2790: a required static key, the same as a literal computed
key, a tuple element) stays silent. Uncertifiable since ADR 0791: an optional
chain, which may skip the delete. A cast root (`delete (store as { v?: number }).v`) is
reported (ADR 0790), from `store` (ADR 0922).

`Rejected.tsx` is TypeScript's and silent: `delete store.v!` and `delete
(store.v as T)` (TS2703), and a link that may be `undefined` or `null`
(TS18048, TS18047).
