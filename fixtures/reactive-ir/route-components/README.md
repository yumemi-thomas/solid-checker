# Audited TanStack routers render route components (ADR 0334)

An audited `@tanstack/solid-router` only renders the `component`, `pendingComponent`, `errorComponent` and `notFoundComponent` values of a route's options (`Match.jsx`: `<Dynamic component={OutComponent()}/>`; `CatchBoundary.jsx`, `renderRouteNotFound.jsx`: `<X/>` or `Dynamic`) and never calls one; router-core reads them only for `.preload?.()` and truth tests. So a function written as such a value of the options literal handed to an exact factory import is a component, and the census of ADR 0331 accounts for that reference.

- `Routes.tsx` (violations): the four keys of `createFileRoute("/")({…})`, `createRootRouteWithContext<…>()({ component })`, and `createRoute({ component })`. Each body read is `SC1001`.
- `Escapes.tsx` (uncertifiable): options that are not a literal at the call, a namespace member `Router.createRoute`, a key the audit did not cover (`loader`), and a local `createRoute` shadowing the import.

Packages are the published bytes: `@tanstack/solid-router@2.0.0-rc.6` (MIT, `sha512-ahTyk3ObuVQtcRFlWbDxe6glsz3KByYoqAYv4iWsLjmiXL+lw2fAuc67dOPQc6nt8AoOKIRy+/VLsMITw+GN8A==`, digest `a5eb41ba…9d63`) and `@tanstack/router-core@1.171.22` (MIT, `sha512-sitsuRkz4qpTjIAV97S5zFCoeGv0OFd6+VWg3ZcIlQbi5R4NIXfz6ogg51n5w6rvTwSODz/lKWb5dl5tvh2bQw==`, digest `f021f4ec…c1b2`). Solid typings are rc.13, as in `rule-negatives`.
