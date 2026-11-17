# A route component throws where its `<Router>` renders (ADR 1026)

`@solidjs/router@2.0.0-next.41` (published bytes, as `route-config`; digest `cec58599…cefcef`) renders a route definition's
`component` with `createComponent` in the route's outlet when the URL matches (ADR 1025's reading), at the `<Router>` tag of
its instance. The owner's ruling: a route component is a conditional render, as `<Show when={x()}><C/></Show>` is, so a plain
throw in its body (`flush()` in a tracked effect) is reported there unless a boundary above every `<Router>` tag takes it.

- Violations: `First` (a bare function) and the inline arrow of `/second`, both top-level definitions of `Router`; `Child`
  and the inline `/child-inline`, children of `/parent`, rendered where `Parent` reads `props.children` in its JSX (the route
  props' `children` getter calls the outlet).
- Uncertifiable: `Guarded` and the inline `/guarded-inline` (the instance's only tag is under an `<Errored>` with a handling
  fallback; main reported the inline one, a binding-less component with no mount found), `CaughtChild` (its parent wraps
  `props.children` in an `<Errored>`), `HandedChild` (its parent hands its props to `Frame`), `Held` (a table held in a
  binding), `Unmounted` (no tag renders its instance).

Chrome: `fixtures/runtime-oracle/route-mount-throw` navigates to each violating route.
