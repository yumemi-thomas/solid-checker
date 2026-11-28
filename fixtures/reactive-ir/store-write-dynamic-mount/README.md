# store-write-dynamic-mount

**Claim (ADR 1234).** A component mounted only through `const D = dynamic(() => C)` and `<D />` (ADR 1016's exact reading,
`stored_throw::dynamic_mount_sites`) starts where `D`'s reached tags are: `dynamic`'s component renders `C` in a memo of its own
(`@solidjs/web@2.0.0-rc.21` `dist/web.dev.js:2451`). SC2003's origin check (`container_root::origin_closed`, ADR 0919) takes that
reference as accounted when the `dynamic` call and every reached tag are outside a function handed to a call that may be a setter, as
it takes a rendered tag of `C` itself. The store is ADR 1232's: a hook of another module creates it per call.

Violations (Chrome STORE_WRITE_OUTSIDE_SETTER, `runtime-oracle/store-write-dynamic-mount`): `Mounted` (a write past the root),
`Closing` (an array mutator; the tag written with a closing tag). On main they are uncertifiable.

Clean: `ThroughSetter` (the write is the setter's own).

Uncertifiable: `InWindow` (the tag is in a function handed to `later`, a call that may be a setter), `OutsideTag` (`D` is also pushed
into an array), `Shadowed` (a local `dynamic`), `Namespace` (`web.dynamic`: the bare-name check refuses it).

Stubs: `stored-throw-dynamic-mount`'s `@solidjs/web` `dynamic`, `store-write-through-module-hook`'s `createStore`.
