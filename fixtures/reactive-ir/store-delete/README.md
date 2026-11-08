# SC2003: `delete` of a store path

`delete store.path` written through a `createStore` read proxy (or a
`createProjection` result) is dropped outside the setter: the proxy's
`deleteProperty` trap warns `STORE_WRITE_OUTSIDE_SETTER` and stores nothing
(`@solidjs/signals@2.0.0-rc.17` `dist/dev.js:5761-5767`). The rule reports it
where it reports the assignment `store.path = x`, with the same exemptions
(ADR 0461).

TypeScript reports `delete` of a required property (TS2790 under
strictNullChecks) and of a readonly one (TS2704), so the operand must carry the
Type Fact `writableOptionalProperty`: the checker resolves the property symbol
and marks it optional and not readonly, which type-checks under every compiler
option for a bare or parenthesized operand. A chain past the root (`store.a.b`) gets the
assignment form's verdict (ADR 0465): silent when a link's domain may hold
`null` or `undefined` (TS18047/TS18048), a violation under strictNullChecks
(this project is strict) when every link excludes both, uncertifiable without
it. A missing fact also makes the delete uncertifiable.

Reported: an optional property of the store deleted in the body, an effect's
apply, inside a wrapper function, a parenthesized operand, inside another
store's setter, through a namespace-imported `S.createStore` (`Namespace.tsx`),
through a projection result, through a `const` alias of the store (ADR
0463), and the nested paths (`store.a.b` in the body and a handler,
`store.o.b` narrowed by `if (store.o)`; rc.17 in Chrome:
`STORE_WRITE_OUTSIDE_SETTER` for all three).

Silent: inside the store's own setter callback (commits, rc.17 probe), on the
setter's draft, on a plain object, through a shadowing parameter. Uncertifiable since
ADR 0829 (Chrome warns, `tsc` accepts; the producer proves the operand neither
optional and writable nor rejected): a non-optional property whose type
includes `undefined` and a computed key here, and in `Unproven.tsx` a literal
computed key, an open record key, an index-signature member, an array index, a
nested computed key, `unknown` and a literal-typed variable key. Its
`RejectedKeys` (TS2790: a required static key, the same as a literal computed
key, a tuple element) stays silent. Uncertifiable since ADR 0791: an optional
chain, which may skip the delete. A cast root (`delete (store as { v?: number }).v`) is
reported since ADR 0790: the flow reads it as the store (Chrome, rc.21, warns).

`Rejected.tsx` is TypeScript's and silent: `delete store.v!` and `delete
(store.v as T)` (TS2703), and a link that may be `undefined` or `null`
(TS18048, TS18047).
