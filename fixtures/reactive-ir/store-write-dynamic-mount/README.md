# store-write-dynamic-mount

**Claim (ADR 1234).** A component mounted only through `const D = dynamic(() => C)` and `<D />` (ADR 1016's exact reading) is
rendered at `D`'s tags: `dynamic`'s component calls `C` in `createMemo(() => untrack(() => component(props)))` (`@solidjs/web@2.0.0-rc.21`
`dist/web.dev.js:2451`, ADR 0992), as `<Dynamic component={C}>` does. Two facts follow:
- the component census (`indexes::census::dynamic_source_value`) takes that reference for a render, so `C`'s identity is proved;
- SC2003's origin check (`container_root::origin_closed`, ADR 0919) takes it as a start at the `dynamic` call and every reached tag
  (`stored_throw::dynamic_mount_sites`), each outside a function handed to a call that may be a setter, as it takes a tag of `C`.

The stores are ADR 1232's (a hook of another module creates one per call) and a component's own.

Violations (Chrome STORE_WRITE_OUTSIDE_SETTER, `runtime-oracle/store-write-dynamic-mount`): `Mounted` (a write past the root),
`Closing` (an array mutator; the tag written with a closing tag), `OwnStore` (the component's own `createStore`). On main they are
uncertifiable.

Clean: `ThroughSetter` (the write is the setter's own).

Uncertifiable: `InWindow` (the tag is in a function handed to `later`, a call that may be a setter), `OutsideTag` (`D` is also pushed
into an array), `Shadowed` (a local `dynamic`), `Namespace` (`web.dynamic`: the bare-name check refuses it).

Stubs: `stored-throw-dynamic-mount`'s `@solidjs/web` `dynamic`, `store-write-through-module-hook`'s `createStore`.
