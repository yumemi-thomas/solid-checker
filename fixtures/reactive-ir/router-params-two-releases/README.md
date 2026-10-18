# Two router copies in one program (ADR 0324)

`packages/a` resolves `@solidjs/router` to the audited `2.0.0-next.26`;
`packages/b` resolves it to a copy whose manifest says `2.0.0-next.18`, a
release the dialect did not audit. The premise reads each import's own
resolved package manifest, so one unaudited copy leaves it unapplied in the
whole project: both `RouteAccessorA` and `RouteAccessorB` keep their
`SC9012`, and the notice (`SC9014`) names the unaudited import. Declarations
as in `router-params` (the `next.18` copy keeps the `next.26` declarations,
which is the point of the version string: it is the only thing that differs).

Both copies are the published `2.0.0-next.26` files; `packages/b` differs only in the version its manifest states.
