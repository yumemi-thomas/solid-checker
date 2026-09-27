# ADR 0128: An undeclared declaration re-export costs only that export

- Status: accepted and implemented (2026-09-27); written with the implementation
- Date: 2026-09-27
- Owners: the resolver's export census (`packages/cli/scripts/artifact-resolution.mjs`,
  `declarationReexportGap`), the certifier's archive replay
  (`rust/crates/solid-facts-backend/src/contract_certification/export_bindings.rs`,
  `declaration_reexport_gap`), the emitter
  (`rust/crates/solid-facts-backend/src/main.rs`, `contract_exports_for_entry_file`)
- Relation: applies the owner principle of the `SC9014` import-scoped gap
  (ADR 0127 § 2, `release_scope.rs`) to certification. Leaves ADR 0027 (core
  re-exports leave the surface) and the strictness of `bind_exports`
  unchanged.

## Context

`solid-js@2.0.0-rc.9`'s `types/index.d.ts` re-exports `$DEVCOMP`,
`createErrorBoundary`, `createLoadingBoundary`, `createRevealOrder` and
`sharedConfig` from `./client/core.js` and `./client/hydration.js`, whose
declarations do not declare them. The runtime exports all five. It is an
upstream typing defect.

The resolver's `bindExport` finds no declaration binding for such a name and
leaves it out of `exports`. The emitter still published it, because it is on
the runtime surface and in the declaration census. `bind_exports` then refused
the whole artifact case: `resolved artifact has no exact runtime/declaration
binding for export "$DEVCOMP"`. Since ADR 0127 the census head installs rc.9, so
every head row certified through the published dependency graph refused at the
`solid-js` graph node and got 0 closure candidates.

## Decision

1. **The gap is a census, named by the resolver and replayed by the
   certifier.** A name is an *unbound declaration export* when it is on the
   runtime/declaration intersection, its runtime binding is exact, and its
   declaration chain ends in a module of the same package that publishes no
   export by that name. The chain can run through `export { x } from`, an
   import that is then exported, or a local `export *`. The resolver records
   these names in the additive `unboundDeclarationExports` field. The field is
   absent when empty, so no other resolution changes by a byte. The certifier
   replays the census from the archive bytes and refuses any disagreement,
   in either direction.
2. **It fails closed wherever the bytes could publish the name.** A module on
   the chain is not proven to lack the name if it does any of these: declares
   the name in any space or spelling (a namespace, an interface, a type alias,
   a type-only specifier), forwards it from outside the package, has an
   `export =`, has an `export *` the walk does not follow (type-only or
   external), or forms a cycle. The name then keeps its existing whole-case
   refusal.
3. **Only the declaration axis.** An ESM runtime re-export of an undeclared
   name fails the module graph at link time. No export of that entrypoint is
   then usable, so a runtime gap still refuses the case.
4. **The emitter leaves an unbound declaration export off the surface.** Each
   such export is unavailable, and every other export keeps its exact
   binding. This is not trust in the resolver's omission. Because the replay
   in (1) runs independently, a resolver that leaves out a bindable name still
   refuses. `bind_exports` stays strict, so a document that names the export
   anyway refuses exactly as before.
5. **Only this shape.** An export the binder cannot bind for other reasons
   keeps refusing its case. Examples: an exported namespace
   (`fixtures/package-contracts/namespace-export-surface`), an external
   re-export with no planned dependency, an ambiguous star. That wider
   question stays open.

## Soundness

Omitting a name puts it in a category that already exists and already fails
closed: names absent from an accepted contract. Runtime-only and
declaration-only names are already in that category (see
`torture-dts-disagreement`), and so are core re-exports under ADR 0027.
Consumers reach such a name in these ways:

- A consumer's named import, or its namespace-member read of the name, finds
  no summary and gets `PackageContractExportMissing` (`SC9005`). The one
  exception is a module whose native vocabulary outranks contracts.
- A dependent package's `export { name } from` finds no accepted binding.
  Resolution and replay both refuse that dependent's case.
- A dependent's `export *` forwards only the dependency's verified surface, so
  the name does not reappear.

The node's other exports are unaffected. The runtime module graph links, their
bindings are replayed exactly, and the export-binding evidence root commits to
exactly the bound set. No digest is forged, because the contract states only
the exports it binds.

The reason is not carried verbatim to consumers. They see "this export has no
effect summary", not the binding text, because the contract format has no slot
for a withheld export. Carrying it would take a contract-format field, and all
schema-version-2 producers and consumers would have to migrate together.

## Consequences

- `solid-js@2.0.0-rc.9`'s graph node publishes every export except the five.
  Graphs that compose it are no longer refused at that node. Measured on
  2026-09-27 (`make contract-coverage-census`): the `$DEVCOMP` refusal is gone
  from every head row, but the memo, rootless, trigger and storage head rows
  still get 0 closure candidates. The next node,
  `@solidjs/web@2.0.0-rc.9`, now refuses for an unrelated reason
  (`acceptance receipt has no locally closed semantic claim`). That is a
  separate wall, and this ADR does not address it.
- The resolution field is receipt identity whenever it is non-empty, because
  it is part of the resolved-import root.
- Pinned by `fixtures/package-contracts/declaration-reexport-gap` (the
  contract publishes `own` alone) and by the certification tests
  `a_declaration_reexport_of_an_undeclared_name_costs_only_that_export`,
  `a_declaration_reexport_gap_is_never_claimed_where_the_bytes_could_publish_the_name`
  and `a_dependent_binds_around_a_declaration_reexport_gap_and_refuses_through_it`.
