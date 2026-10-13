# ADR 0143: An inherited `returns` closure is restated only when it is empty

- Status: accepted and implemented (2026-09-28); written with the implementation
- Date: 2026-09-28
- Owners: the accepted-export projection (`project_export_semantics`,
  `rust/crates/solid-reactive-ir/src/contracts.rs`), the inherited premise
  (`ContractExport::inherited_closure`, `solid-reactive-ir/src/lib.rs`) and,
  through it, the generator's inherited `returns` arm and the certifier's
  re-derivation of it (`inferred_contract.rs`)
- Relation: repairs the inherited premise of the 2026-09-15 closure-gap plan
  (§ 1) for the outputs ADR 0113 and ADR 0115 introduced. The same shape as
  `creates_closed_empty`, which already exists for the same reason.

## Context

A package that re-exports a dependency's function publishes that dependency's
closed domains again, and the certifier discharges them by composition: it
re-runs the generator's derivation over the projection of the dependency's
certified export (`inherited_export_projection`) and admits the parent's claim
when the two agree.

For `returns` the derivation read the projection `ContractExport::returns`. That
is the consumer's single reactive leaf, and since ADR 0113 and ADR 0115 it is
`Known(None)` both for `returns: []` and for a closed claim over outputs that
name no leaf: one `plain` return, a return of an argument, a union of exact
outputs. The inherited arm turned every `Known(None)` into `returns: []` -- the
claim that the export yields **no value**. The certifier's re-derivation made
the same mistake, so the two agreed and composition admitted it.

It surfaced with ADR 0142. `@tanstack/query-core` began to certify
`hashKey` with one `plain` return, and `@tanstack/solid-query`'s graph root,
which re-exports it, proposed `hashKey: returns: []`. The synthesized
empty-return veto called it, got a string, and the contradiction refused the
whole row (`probe contradiction … for claim … hashKey returns`, reproduced on
one row with the certify scratch observed). `@kobalte/core`'s `./colors`
recovery failed the same way on `@solid-primitives/utils`' colour helpers.
Before ADR 0142 the defect was latent, not absent: any re-export of a plain
return whose samples all happened to return `undefined` (`detectColorFormat("x")`)
would have certified a false `returns: []`. A consumer reads both as
`Known(None)`, so no finding could have differed, but the document would have
stated something false.

## Decision

**The inherited `returns` premise holds only when the dependency's accepted
claim is closed *and empty*.** `project_export_semantics` sets
`returns_closed_empty` from the accepted document's own claim, exactly as it
sets `creates_closed_empty`, and `inherited_closure(Returns)` requires it. A
dependency whose `returns` closes over any operation leaves the re-export's
`returns` unproposed: open, with no record, until a later decision restates the
dependency's exact operations.

## Soundness

The change only removes proposals. Every locally inferred summary keeps
`returns_closed_empty: false`, and the local arms (ADR 0035, 0109, 0113, 0115)
never read the inherited premise.

## Consequences

- `only_the_empty_returns_closure_projects_as_closed_empty` (the projection)
  and `an_inherited_plain_return_is_not_restated_as_returns_empty` (the
  generator) pin both halves; the existing inherited-premise test now states
  that its dependency's `returns` is empty.
- `@tanstack/solid-query` and `@kobalte/core`'s `./colors` certify again; their
  re-exports of plain returns are open instead of falsely empty.
- Remaining: restating a dependency's exact return operations (`plain`,
  argument containers) on a re-export would close those domains too. That needs
  the projection to carry the operations and the certifier to discharge each
  one by composition, and is not done here.

## Amendment (2026-09-30): ADR 0170

The remaining item above is done for the exact outputs: the projection now
carries a closed non-empty `returns` claim's operations
(`ContractExport::returns_restated`) and a re-export states them again, so
`@solid-primitives/sse`'s `number` restates `utils`' plain return. The empty
closure is unchanged, and so is the rule that `Known(None)` alone is never read
as `returns: []`.
