# array-shape-v1

This fixture records the `arrayShape` distinctions used by Solid 1.x's
`v1/prefer-for` (SC8014) safe-fix gate. The preference now has an earlier
reactive-input gate, so these declared static/unproven receivers remain clean;
when a reactive receiver reaches the fix gate, only a proven array receives the
`<For each>` rewrite.

`map-receiver-cases.tsx` preserves aliased array, tuple, non-array collection,
and unconstrained type-parameter shapes without turning any of their static
`.map` calls into a preference finding. The armed preference process fixtures
exercise the reactive array fix path.

The `node_modules/solid-js/package.json` stub pins the 1.x dialect; without it
the fixture silently runs the v2 catalog.
