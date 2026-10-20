# Dependency source: a package context another package re-provides (ADR 0289)

Every visible provider vetoes a selected package's context proof when it supplies an inert value (ADR 0300 excludes only unseen providers): `dep-ctx` creates `Ctx` and exports a `Provider` with two signal accessors; selected `dep-wrap` provides a static value via `StaticWrap`; `A` under it reads `api.orientation()`, which returns the static value at run time. Expected: nothing reported (the single visible provider was once trusted, giving a false SC1001). Stub `context-member-identity`'s.
