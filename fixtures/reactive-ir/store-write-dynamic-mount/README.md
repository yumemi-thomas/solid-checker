# store-write-dynamic-mount

**Claim (ADR 1234).** A component mounted only through `const D = dynamic(() => C)` and `<D />` (ADR 1016's exact reading,
`stored_throw::dynamic_mount_sites`) runs where `dynamic`'s component calls it, in `createMemo(() => untrack(() => component(props)))`
(`@solidjs/web@2.0.0-rc.21` `dist/web.dev.js:2451`, ADR 0992), at `D`'s tags. For the store-write rule (SC2003) only:
- the origin check (`container_root::origin_closed`, ADR 0919) takes that reference as a start at the `dynamic` call and every reached
  tag, each outside a function handed to a call that may be a setter, as it takes a tag of `C`;
- the function's identity stays undecided (the `() => C` reference escapes the component census), but a region only such a mount
  enters (`UndecidedRegion::dynamic_mount_only`) withholds no store write's violation: a plain call is the one thing identity
  would rule out, and the mount makes none.

Every other rule keeps the undecided region (a read in such a body is no proven component read: ADR 1234's first cut proved the
identity for all rules and Chrome disagreed on STRICT_READ_UNTRACKED). The stores are ADR 1232's (a hook of another module creates
one per call) and a component's own.

Violations (Chrome STORE_WRITE_OUTSIDE_SETTER, `runtime-oracle/store-write-dynamic-mount`): `Mounted` (a write past the root),
`Closing` (an array mutator; the tag written with a closing tag), `OwnStore` (the component's own `createStore`). On main they are
uncertifiable.

Clean: `ThroughSetter` (the write is the setter's own).

Uncertifiable: `InWindow` (the tag is in a function handed to `later`, a call that may be a setter), `OutsideTag` (`D` is also pushed
into an array), `Shadowed` (a local `dynamic`), `Namespace` (`web.dynamic`: the bare-name check refuses it).

Stubs: `stored-throw-dynamic-mount`'s `@solidjs/web` `dynamic`, `store-write-through-module-hook`'s `createStore`.
