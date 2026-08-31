# A missing target behind a bare-name condition is a refusal, not an inapplicability

`solid` is this checker's own ecosystem default: vite-plugin-solid and
solid-start activate it unconditionally, so `"./unbuilt-solid.jsx"` -- which
this artifact does not ship -- is a target every real Solid consumer reaches.
That is a defective publish, and the census row that selects it must REFUSE.

It must specifically not be recorded `unpublished-conditional-target`. That
disposition says "no consumer reaches a certifiable module here at all", and it
is reserved for a *private namespaced* condition (`@scope/name`,
`vendor/name`), whose namespacing is the published convention for a condition
one tool opts into by name. A bare-name condition -- `solid` here, and equally
`bun`, `workerd`, `edge-light`, `react-native`, `electron` -- is switched on for
every consumer of the ecosystem that owns it, so a missing target behind one is
a real failure. `fixtures/package-contracts/unpublished-conditional-target`
pins the namespaced half of that boundary; this fixture pins the bare half.

The `development` and `default` branches are shipped, so the package still
produces a proposal: the refusal is localized to the `solid` census row, which
is the second half of the claim. A regression that made the row inapplicable
would empty `expected-refusals.json` while `expected.json` stayed green.
