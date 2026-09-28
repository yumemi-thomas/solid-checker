# ADR 0150: A local definition declared by another package costs only that export

- Status: accepted and implemented (2026-09-28); written with the implementation
- Date: 2026-09-28
- Owners: the resolver's export census (`packages/cli/scripts/artifact-resolution.mjs`,
  `exactExportBindings`, `foreignDeclarationOwner`), the certifier's archive
  replay (`rust/crates/solid-facts-backend/src/contract_certification/export_bindings.rs`,
  `verify_snapshot_exports_with_dependencies`, `target_package`), the emitter
  (`rust/crates/solid-facts-backend/src/main.rs`, the declaration surface of
  `emit_package_contract`)
- Relation: the second shape under ADR 0128's owner principle. ADR 0128 covers
  a declaration chain that ends nowhere; this covers one that ends in another
  package while the runtime binding stays in this one. `bind_exports` stays
  strict. Supersedes, for this shape only, the emitter's
  `accepted_declaration_reexport_target` path that bound such a name's two axes
  to two different entities.

## Context

Under host `node` (ADR 0140) the published-dependency graph refused at the
`solid-js@2.0.0-rc.9 . [import,node]` node: *Declaration target for export
"action" is re-exported from dependency "@solidjs/signals"
(module "dist/types/core/action.d.ts"), which no planned dependency binds*.
The `node` condition selects `dist/server.js`, which **defines** `action`
itself. `types/index.d.ts`, shared by every condition, re-exports `action`
from `@solidjs/signals`. The browser build re-exports it from
`@solidjs/signals` on both axes, so there the two agree.

The resolver bound the runtime axis to `server.js`'s own `action` and the
declaration axis to `@solidjs/signals`' declaration. The emitter published the
name, and `bind_exports` refused the whole case. Every graph composed through
that node prepared nothing, and 60 `@solid-primitives` packages fell off the
graph lane under `node` (`docs/package-contract-v2/phase22/2026-09-28-solid-primitives-checkpoint.md`).

The rc.9 server build has this shape for 26 names, all re-exported by
`types/index.d.ts` from `@solidjs/signals` and defined locally in
`dist/server.js`: `action`, `affects`, `configureClientErrors`, `createOwner`,
`createReaction`, `createTrackedEffect`, `deep`, `flush`, `getNextChildId`,
`getObserver`, `getOwner`, `isDisposed`, `isPending`, `latest`, `mapArray`,
`merge`, `onCleanup`, `onSettled`, `reconcile`, `refresh`, `repeat`,
`resetErrorHalt`, `resolve`, `runWithOwner`, `until`, `untrack`. The browser
and host-free builds have none. The twelve names `server.js` re-exports from
`@solidjs/signals` (`$PROXY`, `flatten`, `omit`, ...) are one entity on both
axes and are unaffected.

## Decision

1. **Such an export is dropped, not bound to its runtime definition.** Binding
   the runtime entity and treating the declaration as "only a type" was
   rejected: the declaration describes another package's code, so any claim
   read off it (a signature, a callable path, a value shape) is a claim about
   code this entrypoint does not run. No single entity carries both
   identities, so the export has no exact identity and cannot be described.
2. **The shape, on the bound targets.** A name on the runtime/declaration
   intersection is a *foreign declaration export* when its runtime binding is
   exact and in this package, and its declaration binding is exact and in
   another package. Package identity decides this, not paths or snapshot
   roots: a target reached without crossing an accepted dependency edge is this
   package's, and one reached through an edge belongs to that dependency's
   package. A self-package edge (ADR 0012) is this package's own. The route to
   either target does not matter: a local definition, a local re-export
   chain, a named or star re-export of the dependency.
3. **The resolver names it; the certifier replays it.** The resolver records
   these names in the additive `foreignDeclarationExports` field and leaves
   them out of `exports`. The field is absent when empty, so no other
   resolution changes by a byte. The certifier recomputes the set from the
   archive and the planned dependencies' snapshots
   (`BindingTarget.snapshot_root` → the owning plan's package) and refuses any
   disagreement, in either direction. Validation requires the name to be in
   the declaration census, and in neither `exports` nor the ADR 0128 set.
4. **It fails closed where the binding is not exact.** With no accepted or
   planned dependency for the declaration, no exact declaration binding exists
   and nothing proves the name foreign. The resolver keeps its existing
   refusal (`accepted dependency X has no exact declarations binding for export
   Y`), which is also the hint the graph lane uses to plan X as a
   declaration-axis node (`staticBindingDependencies`). An owner that is not
   exactly one named package proves nothing and keeps the name bound, which the
   replay then refuses.
5. **The emitter leaves the name off the surface.** Each such export is
   unavailable, and every other export keeps its exact binding. A document
   that names it anyway refuses at `bind_exports` exactly as before.
6. **Only this shape.** The converse (a local declaration of a name the
   runtime forwards from another package) and two different foreign packages
   on the two axes keep their existing behavior.

## Soundness

As in ADR 0128, omitting a name puts it among the names absent from an
accepted contract, which already fail closed:

- a consumer's import of the name finds no summary (`SC9005`), except where a
  module's native vocabulary outranks contracts (as it does for `solid-js`);
- a dependent's `export { name } from` finds no binding, and resolution and
  replay refuse that dependent's case;
- a dependent's `export *` forwards only the dependency's verified surface.

The runtime module graph links: the runtime binding is exact, so the node's
other exports are replayed and bound exactly, and the export-binding evidence
root commits to exactly the bound set.

## Consequences

- `solid-js@2.0.0-rc.9 . [import,node]` publishes every export except the
  five ADR 0128 names and these 26. Measured on 2026-09-28 (node host, the
  checkpoint corpus, release binary): packages certified on the graph lane
  under `node` went from 1 to 15; rows refused at the `solid-js` node, 60 to 0;
  exports held behind the unaccepted `@solid-primitives/utils`, 265 to 165.
  Exports clean under `node` did not move (93 of 703).
- The wall moved one node up. 46 rows now refuse at `@solidjs/web@2.0.0-rc.9
  . [import,node]`: its `dist/server.js` re-exports `getOwner`, `untrack` and
  `merge as mergeProps` from `solid-js` (and its typings declare them through
  `types/client.d.ts`), which are three of the 26 names above. By (Soundness)
  that dependent refuses. Clearing it needs a further step this ADR does not
  take: a dependent's re-export of a name its dependency withholds as unbound
  or foreign would itself be withheld, with the dependency's withheld set
  carried in the planned dependency's record and replayed. ADR 0154 took
  that step, for foreign names only (not ADR 0128's).
- ADR 0154 narrows the dependent `export { name } from` line of the
  Soundness list: an exact forward of such a name on both axes is withheld
  instead of refused.
- The resolution field is receipt identity whenever it is non-empty, because
  it is part of the resolved-import root.
- Pinned by `fixtures/package-contracts/foreign-declaration-reexport` (the
  standalone refusal), `scripts/contract-foreign-declaration.test.mjs` (the
  graph-lane contract publishes `own` and `together`, and the resolution
  names `action`), the resolver test `a local runtime definition declared by
  another package's declaration is foreign, alone`, and the certification tests
  `a_local_definition_declared_by_another_package_costs_only_that_export`,
  `a_foreign_declaration_is_never_claimed_where_both_axes_bind_one_entity`,
  `a_dependent_binds_around_a_foreign_declaration_and_refuses_through_it` and
  `resolved_import_root_binds_the_foreign_declaration_export_census`.
