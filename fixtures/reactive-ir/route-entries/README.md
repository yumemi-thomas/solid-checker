# An audited @solidjs/router renders a route component under an owner (ADR 1025)

`@solidjs/router@2.0.0-next.41` copies a route definition's `component` into its route record (`dist/routing.js:369-375`) and
renders it only as `createComponent(component, routeProps)` in the route's outlet (`:902`), which `Routes` calls in a keyed `Show`'s
child under `RouteContextObj` (`dist/routers/components.jsx:157-163`), the route contexts made under `runWithOwner(createOwner(), …)`
(`:63-95`). So, under ADR 0335's premises, a function every reference of which is a JSX tag or such a `component` value, or an inline
arrow written as one, is entered by renders under an owner alone: an owner operation in it has its owner.

- Closed: `Home` (another file), `Layout`, the nested `Nested`, and the inline arrow: their `onCleanup` keeps no
  `owner-callers-unenumerated` row.
- Open (kept uncertifiable): `Shared`, also handed to `setTimeout`; `Preloaded`, whose `preload` member is written (the router
  calls `component.preload?.()` under the route's root, which is no render).

Only "rendered under an owner" is claimed: no violation is reported at a route's mount (a route renders only when the URL matches; ADR
1026). The router is the published `@solidjs/router@2.0.0-next.41` byte for byte, as in `route-config` (MIT; digest `cec58599…cefcef`).
Variants: `route-entries-shadowed` (a local `createRouter`), `route-entries-namespace` (a namespace import, which ADR 0335 refuses).
