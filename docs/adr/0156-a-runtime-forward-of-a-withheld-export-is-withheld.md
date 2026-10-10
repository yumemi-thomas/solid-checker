# ADR 0156: A runtime forward of a withheld export is withheld

- Status: accepted and implemented (2026-09-29); written with the implementation
- Date: 2026-09-29
- Owners: the resolver's export census (`packages/cli/scripts/artifact-resolution.mjs`,
  `prunedDependencyExport` in `bindExport`, `exactExportBindings`), the graph
  lane's orchestration (`packages/cli/scripts/certify-contract.mjs`,
  `prunedDependencyRecords`, `withPrunedDependencyNodes`), graph planning
  (`rust/crates/solid-facts-backend/src/contract_certification/dependencies.rs`,
  `plan_published_contract_graph_with_pruned`), the pruned-node evidence
  (`CertificationPlan::pruned_dependency_evidence`), the certifier's archive
  replay (`export_bindings.rs`, `forwarded_withheld`), the emitter
  (`rust/crates/solid-facts-backend/src/main.rs`)
- Relation: widens ADR 0154 on the runtime axis, and makes a dependency node
  that ADR 0129 pruned something a dependent can be proved to forward from,
  without making it an accepted dependency. Keeps ADR 0128's choice: a
  refusal costs only the export.

## Context

After ADR 0154, `@solidjs/web@2.0.0-rc.9 . [import,node]` still refused, one
name further up: its server build has `export { ssrScope as scope } from
"solid-js/internal"`. The `solid-js ./internal [import,node]` node proposes
nothing, so ADR 0129 prunes it and `@solidjs/web` has no planned dependency for
that specifier. `@solidjs/web`'s typings do not forward `scope` either. They
declare it themselves: `types/index.d.ts` → `export * from "./client.js"` →
`export declare function scope<T extends () => any>(fn: T): T;`. 47 rows of the
checkpoint corpus stopped at that node under `node`.

Binding `scope` to the pruned node was rejected: that node has no claim, so
an export bound to it would be published with nothing backing it.

## Decision

1. **The shape.** A name on the runtime/declaration intersection is a
   *runtime-withheld export* when two things hold. First, its runtime chain
   forwards, through exact named re-export chains, exactly one *withheld
   export* of a dependency. That is either
   - a name a planned dependency's verified replay withheld: ADR 0150, ADR 0154,
     or this rule; or
   - an *own exact export* of a dependency node ADR 0129 pruned: a name whose
     runtime and declaration bindings both lie in the pruned node's own
     package.
   Second, its declaration either forwards the same withheld name or binds
   exactly, wherever that binding lives: a local declaration, another
   module, or a bound export of a dependency. Such an export leaves the
   surface. It is withheld, never bound.
2. **The two shapes this covers.** The pruned-node forward, whether the
   declaration forwards the same name or is the dependent's own
   (`@solidjs/web`'s `scope`). And the reverse of ADR 0150 over a planned
   dependency: the runtime forwards a name the dependency withholds while the
   declaration is local. ADR 0154's shape (both axes forward the same name of
   a planned dependency) keeps its own field.
3. **What stays as it was.**
   - A runtime forward of a *bound* dependency export beside a local
     declaration is not withheld. It keeps its existing binding: this rule
     only withholds names a dependency already withheld.
   - A declaration that forwards a different withheld name, or has no
     binding, keeps its refusal.
   - A pruned node's name that the node itself forwards from another package
     is not its own exact export, so a forward of it keeps its refusal.
   - A withheld declaration beside an exact runtime binding keeps its
     refusal.
   - A dependency that refused, rather than being pruned, still refuses its
     dependents (the orchestrator's cascade). It produces no plan, so no
     evidence exists that could withhold a forward.
4. **Every premise is replayed.** The resolver names these exports in the
   additive `runtimeWithheldExports` field. The certifier recomputes the field
   and refuses a disagreement in either direction. Each premise is replayed:
   - *The runtime module graph links.* For a planned dependency, its own
     replay proved that runtime binding exact before withholding the name.
     For a pruned node, see the next item.
   - *The pruned node.* It is transported into the native transaction
     (`graphCaseSet.cases[].pruned`) with its archive, and planned from its
     bytes exactly as any node is: snapshot, resolution, closure, export
     replay, lock selection. That plan must prove it states nothing (no
     closed claim, closure candidate, positive operation, positive fact or
     initialization claim), or the transaction refuses the forged prune. Its
     evidence carries its own import identity and conditions, and the names
     its own package exports exactly. A dependent's replay selects it by
     specifier and closure-entry importer, and only when no planned
     dependency answers that specifier.
   - *The declaration exists.* The replay binds the declaration over the
     bytes.
   The pruned node is then no node of the graph: it is never finalized,
   composed or bound. Neither is a node planned only because a pruned node
   depends on it.
5. **It composes.** Runtime-withheld exports are part of the dependent's
   withheld set (`SnapshotVerifiedExports::withheld`, the planned-dependency
   record's `withheldExports`), so ADR 0154 and this rule both apply to a
   package that forwards them.
6. **The orchestrator's record is a claim.** A dependent is generated with
   `prunedDependencies` (`{ packageName, exports }` per pruned direct
   dependency: that node's own exact exports). The record never reaches the
   certifier, which reads only the replayed evidence.

## Soundness

Withholding only removes names, which then fail closed exactly as ADR 0128
lists. The dependent's runtime module graph links: every forwarded name has a
replayed exact runtime binding in its dependency. Every other export of the
dependent keeps its exact binding. Nothing binds through a claimless node, so
no export is published without claims backing it.

## Consequences

Measured on 2026-09-29, node host, release binary, base `e338bc48` → this
change (`make primitives-checkpoint`, `make certification-metric` with the
node host):

- `@solidjs/web@2.0.0-rc.9 . [import,node]` certifies. The graph-lane walls
  under `node` are now exactly the host-free ones (`@tauri-apps/api`, a
  `./web` specifier and two `@solid-primitives/utils` installs).
- The checkpoint under `node`: packages on the graph lane 15 → 63, rows
  refused at `@solidjs/web` 47 → 0, exports clean 92 of 703 → 95 of 721 (the
  host-free figure). `@solid-primitives/form` now certifies under `node`.
- The certification metric under `node`: 31 of 905 exports clean → 44 of
  957, equal to host-free.
- The walls next in line are the host-free ones: probe recipes for `reads`
  (299 exports), `recursive-value-shape` (220), `returns` never proposed
  (207), `callbacks` never proposed (193).

- The resolution field is receipt identity whenever it is non-empty.
- Pinned by `scripts/contract-pruned-forward.test.mjs` (the published rc.9
  `@solidjs/web` under `node`: `scope` withheld, `getOwner`, `mergeProps` and
  `untrack` withheld under ADR 0154, over 100 exports bound, and the old
  refusal without a pruned record or with a record that omits the name), by
  the resolver tests `a forward of a name the dependency withholds as foreign
  is withheld too, alone` (the reverse shape) and the certification tests
  `a_runtime_forward_of_a_pruned_export_is_withheld_in_both_shapes`,
  `a_runtime_forward_of_a_withheld_name_is_withheld_whatever_declares_it` (the
  reverse shape, and a bound dependency export beside a local declaration that
  is not withheld), `a_forged_prune_refuses_the_graph` and
  `a_refused_node_is_not_a_pruned_node`.
