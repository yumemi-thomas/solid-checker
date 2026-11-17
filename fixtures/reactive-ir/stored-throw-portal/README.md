# stored-throw-portal

**Claim (ADR 1043).** `Portal` reads `children` only in its own memo, under an owner below the element (`Dialect::component_entry`, ADR 0992), so a component in its children renders in place: a plain throw in its body (`useContext` with no provider and no default, ADR 0893) ends where the `<Portal>` element's does, and the render around it reports it.

Violations (SC4002): `PortalChild`, `NamespacePortalChild` (`<Web.Portal>`), `CalledChild` (`{make()}` in the children).

Uncertifiable: `BoundaryAbovePortal` (an `Errored` above takes it), `SpreadPortal` (a spread may replace `children`), `MountProp` (`mount` is no slot the Portal renders in place).

Chrome: `fixtures/runtime-oracle/stored-throw-portal`. Stubs: `stored-throw-dynamic-mount`'s, with `@solidjs/web`'s `Portal` declaration (byte-faithful). `tsc --noEmit` clean.
