# A local runtime definition declared by another package's declaration

`dist/index.js` defines `action` itself, while `types/index.d.ts` re-exports
`action` from `declaring-package`. The two axes name different entities: the
types describe `declaring-package`'s code, and the runtime runs this package's
own. This is the shape of `solid-js@2.0.0-rc.9`'s server build
(`dist/server.js`, selected under `node`), which defines `action` and 25 more
names locally while `types/index.d.ts` re-exports them from `@solidjs/signals`
(ADR 0150). `together` is the control: both axes forward it from
`declaring-package`, so it is one entity. `own` is this package's on both axes.

What this corpus entry pins: generated **standalone**, with no accepted
dependency, the case refuses exactly as before, with `accepted dependency
declaring-package has no exact declarations binding for export action`. That
reason is the discovery hint `staticBindingDependencies` reads to plan
`declaring-package` as a declaration-axis graph node; without an exact
declaration binding nothing proves the name foreign, so nothing is withheld.

With `declaring-package` planned (the published-graph lane), ADR 0150 applies:

- the resolver names `action` in `foreignDeclarationExports` and leaves it out
  of `exports`;
- the emitter leaves it off the surface, so the contract publishes `own` and
  `together` alone;
- certification replays the census from the archive and the planned
  dependency's snapshot and refuses a resolver that disagrees in either
  direction.

That half is pinned by `scripts/contract-foreign-declaration.test.mjs`, which
generates this fixture's bytes against a private proposal dependency, and by
the certification tests
`a_local_definition_declared_by_another_package_costs_only_that_export`,
`a_foreign_declaration_is_never_claimed_where_both_axes_bind_one_entity` and
`a_dependent_binds_around_a_foreign_declaration_and_refuses_through_it`.

No stub is involved: `node_modules/declaring-package` is the fixture's own
dependency, and nothing here reports a diagnostic.
