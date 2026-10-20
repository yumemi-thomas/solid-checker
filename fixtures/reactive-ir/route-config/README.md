# Audited @solidjs/router renders route definitions (ADR 0335)

An audited `@solidjs/router` (`2.0.0-next.16`, `next.18`, `next.26`) copies a route definition's `component` into its route record and renders it only as `createComponent(component, …)` in the outlet (`dist/routing.js:968-969` here); `preloadRoute` reads `component.preload` only, and `createBranches` treats `children` arrays the same way. So a function written as the `component` of an object-literal definition in `createRouter({ routes: [...] })`, an exact named import, or in such a definition's `children`, is a component.

- `Routes.tsx` (violations): `Home`, `Layout` and the nested `Nested`.
- `Escapes.tsx` (uncertifiable): definitions held in a constant, a definition built by a project call, a `component` key of a nested object that is no definition (`info`), and a namespace member `Routing.createRouter`.

The router is the published `@solidjs/router@2.0.0-next.26` byte for byte (MIT; digest `e67af539…04d6`), as in `router-params-stock-history`. Solid typings are rc.13, as in `rule-negatives`.
