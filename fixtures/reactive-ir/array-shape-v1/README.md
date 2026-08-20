# array-shape-v1

This fixture pins the `arrayShape` fact at its remaining Solid 1.x consumer,
`v1/prefer-for` (SC8014). Every single-callback `.map()` in JSX reports, but the
`<For each>` rewrite is offered only when the receiver is a proven array.

`map-receiver-cases.tsx` covers aliased arrays and tuples, which receive the
fix, plus a non-array collection and an unconstrained type parameter, which
remain report-only. This keeps the autofix fail-closed without relying on
rendered type text.

The `node_modules/solid-js/package.json` stub pins the 1.x dialect; without it
the fixture silently runs the v2 catalog.
