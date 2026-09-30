# ADR 0170: A re-export restates its dependency's closed `returns`

- Status: accepted and implemented (2026-09-30); written with the implementation
- Date: 2026-09-30
- Owners: the projection (`restatable_returns`, `project_export_semantics`,
  `rust/crates/solid-reactive-ir/src/contracts.rs`; the field
  `ContractExport::returns_restated`; the predicate
  `Operation::is_bare_return`, `contract_semantics.rs`) and the generator's
  inherited `returns` arm (`restated_returns`, `inferred_contract.rs`)
- Relation: the "Remaining" item of ADR 0143, for the outputs it named. No
  certifier change: the composition arm of ADR 0151/0128 lineage
  (`census_inherited_dependency_closure`) re-derives the projection with the
  same code and discharges the claim against the dependency's receipt. No
  handshake protocol, no Type Facts field, no contract-format change.

## Context

`@solid-primitives/sse` re-exports `number` from `@solid-primitives/utils`
(`import { … } from "@solid-primitives/utils"; export { … }`, through a sibling
barrel: ADR 0169). At the lead tip `number` is clean in `utils` and, in `sse`,
sat behind `returns never proposed`. The node record showed no widening and
no refusal: the re-export simply proposed nothing for `returns`.

The place it was dropped is the generator's inherited `returns` arm. A
re-export is projected from the dependency's accepted export
(`project_export_semantics`), and the projection's `returns` is the consumer's
single reactive leaf, `Known(None)` for `returns: []` and also for a closed
claim over `plain`, an argument or an argument container. ADR 0143 made the
arm restate only the *empty* closure, because reading `Known(None)` as `[]`
stated that the export yields no value. Everything else (`utils`' `number`:
one closed `plain` return) was left unproposed, and it said why: restating the
exact operations "needs the projection to carry the operations and the
certifier to discharge each one by composition".

The second half already exists. The certifier discharges an inherited closure
by re-running the generator's own derivation over the dependency's certified
export and requiring the parent's claim to *equal* it
(`same_domain_claim`: same closure state, same items, same operations, same
resources), then binds the dependency's claim id as an obligation its receipt
must satisfy. So carrying the operations through the projection is sufficient:
the certifier re-derives the same operations.

## Decision

1. **The projection carries the operations.** `ContractExport::returns_restated`
   is the accepted document's own closed, non-empty `returns` items, when every
   item is a *bare* `return` (`Operation::is_bare_return`: unguarded, at and
   triggered by the call, same stack, untracked, default owner and cardinality,
   no input, resource, provenance or protocol) whose output means the same in
   any package: `plain`, `undefined`, an argument, a fresh array of arguments,
   an invocation result, a props merge, an array of plains, or an ADR 0145
   described callable none of whose callbacks names a resource. All or nothing:
   one other item restates none, because the rest would state a smaller
   enumeration than the dependency certified. Empty for every locally inferred
   summary, for the empty closure (ADR 0143's flag owns it), for an open or
   partial claim, and for a reactive leaf.
2. **The generator states them.** An inherited summary carrying them proposes
   each operation again under its own names (`…:<export>:operation:return`, or
   `return-<n>`), completes `returns` over them, and proposes the closure. It
   is ordered after every local proposal, all of which require
   `inherited_from` to be `None`, and after ADR 0143's empty closure. The
   existing `Known(Some(leaf))` arm (a dependency returning one reactive leaf)
   is unchanged.
3. **Anything that is not an exact forward is untouched.** A local wrapper has
   no `inherited_from`; a renamed default or an ambiguous star never reaches the
   projection (the resolver and ADR 0128/0154/0156 decide those first).

## Soundness

- *Never stronger.* The copied operations equal the dependency's up to name, by
  construction of the bare predicate and the exact-output list. The
  certifier's comparison is against the same derivation over the dependency's
  own certified export, so a parent that says less (`returns: []`), more (a
  different output) or anything over an open dependency is refused (tests
  below).
- *Per host and environment.* The claim is discharged against the
  dependency node's receipt of the same transaction, so it inherits that
  node's host and environment; a dependency that withholds the claim after the
  parent relied on it withholds the parent's closure by name
  (`composed_from_withheld_dependency`).
- *The parent's own census still runs.* The restated `return` is a positive
  operation of the parent, decided by the parent's census over the same
  authenticated bytes. Where it refuses, the operation is withdrawn by name and
  the domain stays open; that is the outcome for `sse`'s `json`, `lines`,
  `ndjson` and `safe`, whose dependency's own census refuses them with the same
  cause (`recursive-value-shape`).

## Consequences

- Pinned by: the projection test `only_an_exact_closed_returns_claim_projects_as_restatable`
  (`contracts.rs`); the generator tests
  `an_inherited_plain_return_is_restated_as_the_dependencys_own_operation` and
  `a_restatement_is_read_only_of_an_inherited_summary_that_carries_one`; the
  composition tests `a_reexport_restates_its_dependencys_closed_plain_return`
  and `a_reexport_restates_only_the_return_its_dependency_certified`
  (contract_certification.rs); the graph end to end
  `a_reexport_restates_its_dependencys_closed_returns_end_to_end` over
  `fixtures/package-contracts/dependency-reexport-returns` (a certified
  forward restates, a forward of a withheld plain return stays open); and the
  generator-level pins in `scripts/contract-dependency-reexport.test.mjs` (the
  restated operation equals the dependency's; a local wrapper of the same
  dependency function proposes no `returns`). The generator-level re-export pin
  fails against a binary built without this change.
- **Historical branch measurement** with `make primitives-checkpoint`
  (release), before and after at
  base `6c78aca5`, 97 probes per host: exports clean host free 100 → 101,
  `browser` 100 → 101, `node` 130 → 131. The one export is
  `@solid-primitives/sse` `number`. All three host measurements completed,
  but the branch's end-to-end checkpoint was interrupted (exit 143);
  these counts do not establish a completed misuse-ledger or tier run.
  Integrated regeneration at source `6208b85f` confirms 101 / 101 / 131
  clean exports, the same one-export gain per host. The freshly bundled tier
  authenticates, and the complete ledger rerun against its rebuilt release
  checker passes two cases (including the separate consumer `access` fix).
  The full package checkpoint remains 1/97; details are in
  `../package-contract-v2/phase22/2026-09-30-primitives-integrated-checkpoint.md`.
- **Demand in the checkpoint is ten exports.** An analysis of every entrypoint
  of the retained trees finds 10 exports that are exact forwards, runtime and
  declaration, of another package's export, all of them in corpus dependencies
  and all on the published-graph lane: `sse` (six, from `utils`) and `form`
  (four, from `a11y`). Nine forward an export that is clean (2) or partial (7)
  in its own package. One moved. The other eight stay behind walls that are
  the dependency's own or the parent's own census, not a missing restatement:
  - `json`, `lines`, `ndjson`, `safe`: `returns` is now proposed and refused
    with the dependency's cause (`recursive-value-shape`);
  - `pipe`: clean in `utils`, but `sse`'s `callbacks` inherited closure is "not
    the projection of" `utils`' (a callbacks re-derivation mismatch that
    predates this change), and its restated described callable is refused at
    the parent's `callable-path` census;
  - `form`'s four `a11y` forwards: `FormControlContext` and
    `createFormControlInput` are degenerate or never propose `returns` in
    `a11y` itself, `createFormControl` is proposed but not certified there, and
    `useFormControl` is refused with `recursive-value-shape` there, so there is
    no closed `returns` to restate (the last, like `json`, now carries the
    dependency's cause).
- The plain lane (a cited dependency, ADR 0151) is not reached: the adapter
  never cites a specifier the case re-exports, a citation carries no per-export
  bindings, and no plan of the cited dependency exists for the composition arm.
  No forwarder in the checkpoint is on that lane.

## Still open

- The parent's positive-operation census for a restated described callable
  (`callable-path`) and the `callbacks` re-derivation for `pipe`.
- A dependency's `Reactive`/`Store` returns and object/tuple shapes with
  resources: not restated, because their meaning names the dependency's own
  resources.
