# SC2003: `delete` of a store path

`delete store.path` written through a `createStore` read proxy (or a
`createProjection` result) is dropped outside the setter: the proxy's
`deleteProperty` trap warns `STORE_WRITE_OUTSIDE_SETTER` and stores nothing
(`@solidjs/signals@2.0.0-rc.17` `dist/dev.js:5761-5767`). The rule reports it
where it reports the assignment `store.path = x`, with the same exemptions
(ADR 0460).

TypeScript reports `delete` of a required property (TS2790 under
strictNullChecks) and of a readonly one (TS2704), so the operand must carry the
Type Fact `writableOptionalProperty`: the checker resolves the property symbol
and marks it optional and not readonly, which type-checks under every compiler
option.

Reported: an optional property deleted in the body, an effect's apply, a
handler, inside a wrapper function, inside another
store's setter, through a namespace-imported `S.createStore` (`Namespace.tsx`),
and through a projection result.

Silent: inside the store's own setter callback (commits, rc.17 probe), on the
setter's draft, on a plain object, through a shadowing parameter. Refused, also
silent: a non-optional property whose type includes `undefined`, a computed
key, an optional chain, a cast root and a `const` alias of the store, which
the assignment form does not follow either (docs/open-work.md).
