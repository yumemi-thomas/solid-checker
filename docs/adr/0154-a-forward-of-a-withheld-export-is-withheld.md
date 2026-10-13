# ADR 0154: A forward of a withheld export is withheld

- Status: accepted and implemented (2026-09-28); written with the implementation
- Date: 2026-09-28
- Owners: the resolver's export census (`packages/cli/scripts/artifact-resolution.mjs`,
  `withheldDependencyExport` in `bindExport`, `exactExportBindings`), the
  graph lane's planned-dependency record (`packages/cli/scripts/certify-contract.mjs`,
  `mergeProposalDependencies`, `withheldExportsOf`), the certifier's archive
  replay (`rust/crates/solid-facts-backend/src/contract_certification/export_bindings.rs`,
  `forwarded_withheld`, `SnapshotVerifiedExports::withheld`), the emitter
  (`rust/crates/solid-facts-backend/src/main.rs`)
- Relation: the next step ADR 0150 named. ADR 0150's Soundness said a
  dependent's `export { name } from` of a withheld name refuses that
  dependent's case; this narrows that one sentence for exact forwards and
  leaves every other shape as it was.

## Context

After ADR 0150, `solid-js@2.0.0-rc.9 . [import,node]` withholds 26 names as
foreign declaration exports. `@solidjs/web@2.0.0-rc.9`'s server build
(`dist/server.js`) re-exports three of them, `getOwner`, `untrack` and
`merge as mergeProps`, from `solid-js`. Its typings forward the same three
names from `solid-js` too, through `types/index.d.ts` → `export * from
"./client.js"` → `import { getOwner, untrack, merge as mergeProps } from
"solid-js"`. The resolver found no binding in the planned `solid-js` node and
refused the whole `@solidjs/web [import,node]` case. That stopped 47 rows of
the checkpoint corpus under `node`.

## Decision

1. **The shape.** A name on the runtime/declaration intersection is a
   *forwarded foreign export* when both axes reach, through exact named
   re-export chains, the same export name of the same planned dependency, and
   that dependency's verified replay withheld that name as a foreign
   declaration export (ADR 0150) or as a forwarded foreign export (this rule),
   binding nothing by it. The dependent's export is that unavailable export,
   so it leaves the dependent's surface too. A renamed forward
   (`merge as mergeProps`) is the same export under another name.
2. **Exact chains only.** A chain is followed through `export { x } from`,
   an import that is then exported, a local re-export, and a local `export *`
   when every star that reaches the name reaches this same withheld name and
   none binds it. Nothing else widens:
   - a name the dependency never had, or withholds for another reason
     (ADR 0128, an unaccepted dependency, a dependency node that left the
     graph under ADR 0129), keeps its refusal;
   - a local definition or declaration of the name on either axis keeps its
     refusal, and so do two different withheld names on the two axes;
   - a forward through an external `export *` is unaffected: a star forwards
     only the dependency's verified surface, which has no withheld name.
3. **The dependency's record carries its withheld set.** A planned
   dependency's record in the graph lane (`proposalDependencies[specifier]`)
   gains `withheldExports`, the dependency resolution's foreign and forwarded
   foreign exports. It is absent when empty, so no other record changes by a
   byte, and it is part of the resolution session key. The record is the
   orchestrator's claim, not a proof: the resolver uses it only for a name the
   record binds nothing by.
4. **The certifier recomputes it.** Each verified plan now keeps the set its
   own replay withheld (`SnapshotVerifiedExports::withheld`), recomputed from
   that dependency's bytes. The dependent's replay reads the planned
   dependency's recomputed set, never the record. The resolver names the
   forwards in the additive `forwardedForeignExports` field. The replay
   recomputes that list and refuses any disagreement in either direction, so
   a forged or stale record refuses. Validation requires each name to be in
   the declaration census and in none of `exports`, the ADR 0128 set and the
   ADR 0150 set.
5. **It composes.** A dependent's forwarded foreign exports are part of its
   own withheld set, so a package that forwards the dependent's name
   withholds it in turn.
6. **The emitter leaves the name off the surface**, as for ADR 0128 and ADR
   0150. `bind_exports` stays strict.

## Soundness

The forwarded name is, on both axes, the dependency's export of that name.
That export has no exact identity (ADR 0150), so this one has none either.
Omitting it puts it among the names absent from an accepted contract, which
fail closed exactly as ADR 0128 lists. The dependent's runtime module graph
links, because the dependency's runtime does export the name: the
dependency's replay proved that runtime binding exact before withholding the
name. Every other export of the dependent keeps its exact binding.

## Consequences

Measured on 2026-09-28, node host, release binary, base `35f5b153`:

- `@solidjs/web@2.0.0-rc.9 . [import,node]` now resolves past `getOwner`,
  `mergeProps` and `untrack`. It still refuses, one name further up, and so
  the checkpoint's node numbers do not move (47 rows refused, 15 packages on
  the graph lane, 92 of 703 exports clean, before and after). The new first
  refusal is `export { ssrScope as scope } from "solid-js/internal"`: the
  `solid-js ./internal [import,node]` node states nothing and proposes nothing,
  so ADR 0129 takes it out of the graph and `@solidjs/web` has no planned
  dependency for that specifier. With that node in place, resolution of
  `@solidjs/web` completes (109 exports, the three names withheld).
- The certification metric under `node` went from 30 to 31 clean exports, and
  from 885 to 905 in scope. `@kobalte/core@2.0.0-alpha.2` depends on
  `@solidjs/web@2.0.0-rc.3`, whose server build has the same three forwards
  and no `solid-js/internal` re-export. That node now certifies under
  `[import,node]` and `[import,node,solid]`, and with it the `./i18n`
  entrypoint's graph case (+20 exports, +1 clean). This is the rule
  replayed end to end on published bytes.
- The resolution field is receipt identity whenever it is non-empty.
- Pinned by `scripts/contract-forwarded-foreign.test.mjs` (the published
  rc.9 `@solidjs/web` under `node`: `getOwner`, `mergeProps` and `untrack`
  withheld, `createComponent` bound, and the old refusal without the
  record's census), by the resolver test `a forward of a name the dependency
  withholds as foreign is withheld too, alone`, and by the certification
  tests `a_forward_of_a_withheld_name_costs_only_that_export` (renamed,
  transitive, and a renamed forward of a bound name that binds exactly),
  `a_forward_of_a_withheld_name_is_replayed_never_trusted` (stale, forged,
  shadowed, mixed and never-had cases) and
  `resolved_import_root_binds_the_forwarded_foreign_export_census`.
