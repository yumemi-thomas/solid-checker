# ADR 0168: Argument-scoped `reads` rows for the dialect primitives

- Status: accepted and implemented (2026-09-30); written with the implementation
- Date: 2026-09-30
- Owners: the dialect (`RowScope::Arguments`, `ArgumentScope`,
  `argument_row`; `rust/crates/solid-dialect/src/lib.rs`, `solid_2.rs`), the
  implementation census (`census_dialect_axiom`, `census_arguments_terminator`,
  `census_argument_premises`, `census_invoked_slot_is_attributable`,
  `census_delegated_denials`;
  `rust/crates/solid-facts-backend/src/contract_certification/type_facts.rs`),
  the Type Facts producer (`ImplementationCall.argumentsPrimitiveSyntax`;
  handshake protocol 74) and the audit
  `docs/package-contract-v2/audits/2026-09-30-solid-2-rc9-reads-rows-for-dialect-primitives.md`
- Relation: fills what [ADR 0165](0165-the-reads-census-walks-calls.md) § Consequences
  left to "the audits' to add, one at a time", and takes ADR 0146's argument
  condition ("every argument of the creating call is a primitive by grammar") from a
  premise about a returned accessor to a premise about a dialect call.

## Context

ADR 0165 made the `reads` census walk calls. A call to a dialect primitive with no
audited `reads` row refuses every export that reaches it, whatever the primitive
does. The 2026-09-30 census (`make contract-coverage-census`, the pinned 16-package
corpus, host-free, release binary, base `816d6f58`) names them. Counting the
callee the walk refuses first, per withheld `reads` closure record:

| Primitive (declaration) | Records |
| --- | --- |
| `createSignal` (`solid-js`, `types/client/hydration.d.ts`) | 16 |
| `onCleanup` (`@solidjs/signals`) | 9 |
| `getOwner` (`@solidjs/signals`) | 9 |
| `createMemo` (`solid-js`) | 3 |
| `useContext` (`solid-js`) | 2 |
| `createComponent` (`solid-js`) | 1 |

The first refusal hides the rest of a walk, so `untrack`, `runWithOwner` and
`createRoot` are counted as zero by this instrument and named by ADR 0165 itself.
The primitives checkpoint (97 packages, host-free `measure.json`) ranks the same
primitives by exports blocked: `solid-js:createSignal` 83, `solid-js:createMemo` 68,
"callee in `solid-js`" 45, "callee in `@solidjs/signals`" 17, `createEffect` 11.

## Decision

1. **Seven rc.9 `reads` rows, read on the published bytes** (the audit above),
   in every build each `exports` map can select:

   | Row | Scope |
   | --- | --- |
   | `@solidjs/signals` `getOwner`, `onCleanup` | every condition |
   | `solid-js` `useContext` | every condition |
   | `@solidjs/signals` `untrack` | arguments: slot 0 is invoked |
   | `@solidjs/signals` `runWithOwner` | arguments: slot 1 is invoked |
   | `@solidjs/signals` `createSignal` | arguments: slot 0 is primitive or attributable |
   | `solid-js` `createSignal` | arguments: slot 0 is primitive; delegate signals `createSignal` |

   Each was probed as the compute of a tracking memo on the real bytes, as ADR
   0163's synthesized veto does; the audit records the results and the negatives.

2. **`RowScope::Arguments(ArgumentScope)`.** A row that holds only for calls whose
   arguments satisfy a scope, stated by the dialect and replayed by the census at the
   call site. It is the third scope beside `EveryCondition` and `HostTarget`, and,
   like `HostTarget`, is invisible to every caller that has no call site:
   `denies`, `primitive_performs_no_operation` and `some_audit_denies_primitive`
   never answer for it, and `argument_row` is its one entry. The scope has three
   premises and a delegate list:

   - `primitive_slots`: the slot is a primitive by grammar. It rests on the
     producer's per-call `argumentsPrimitiveSyntax` (below). A slot not written, a
     spread, or a producer that states nothing is not proved, and refuses. This is
     ADR 0146's condition, and for `solid-js` `createSignal` it is what keeps the
     call out of the hydration gate (a function first argument reaches `withHydrationGate`,
     whose compute reads a signal the call created) and out of the server memo.
   - `invoked_slots`: the archive's own code invokes the slot's callable. The
     callable's reads are its author's, and the census can attribute them only when
     the slot holds a function literal inside the implementation under the walk (its
     calls are walked with the frame) or a value rooted at a parameter of the
     censused export (the caller's own callable, and at depth 0 only, ADR 0165 § 3).
     A callable passed by reference -- a module-level helper, or an accessor of a
     memo this very call created -- is refused. The producer facts are the ones the
     standard-library invoker rule already consumes (`argument_parameters`,
     `argument_callables`); the rule is shared, not copied.
   - `callable_or_primitive_slots`: either of the above, for a primitive that takes a
     function path and a plain path and whose audit cleared both (`createSignal`).
   - `delegates`: as for `HostTarget`, calls the reading followed into another
     archive. A delegate an every-condition row denies binds as before. One that only
     an argument-scoped row denies binds when that row's premises hold for the
     delegating call's own arguments; a row may delegate this way only where the archive
     forwards them, and the audit section says so (`solid-js` `createSignal` forwards
     `fn` and `second` to the signals export unchanged).

3. **The producer states the primitive fact per call.** `ImplementationCall`
   gains `argumentsPrimitiveSyntax: bool[]`, one entry per written argument, from the
   same syntactic test ADR 0146 uses for a call-result source's arguments
   (`primitiveBySyntaxLocked`); a spread is `false`. Handshake protocol 74. Absence
   proves nothing and the census reads it that way.

4. **What is unchanged.** The forms half of the `reads` census, ADR 0165's walk and
   its refusals, the flat rows already shipped (see Remaining approximations), and every
   other domain.

## Consequences

- `untrack(() => sig())` certifies `reads: []` when `sig` is the caller's parameter
  (the call is the row's, the literal's own call is the caller's).
  `createSignal(() => sig())` from `solid-js` does not: its row holds only for a
  primitive first argument, and it declines by name
  (`argument 0 is not a primitive by its grammar`). `createSignal(0)` certifies.
  These are pinned in `type_facts.rs`
  (`the_reads_walk_admits_untrack_of_a_literal_and_refuses_create_signal_of_a_function`,
  over synthesized transcripts: the native tracer harness cannot stand up an audited
  `solid-js` archive, as ADR 0146 recorded) with the row-level tests
  (`census_dialect_axiom_binds_the_rc9_reads_rows_and_their_argument_premises`,
  `census_argument_premises_state_what_the_producer_proved`) and the dialect tests
  (`argument_rows_state_checkable_premises`, and the derivation and citation tests,
  which now count 118 rows and 27 further `Implementation` citations).
- The producer fact is pinned in Go by a fixture-backed test over real transcripts
  (`TestCallStatesItsArgumentsPrimitiveSyntax`).
- Measured, and what did not move, in the section below.

## Remaining approximations

- **The flat rows already shipped.** `createMemo`'s row (and the other flat rc.9
  `reads` rows) denies the archive's own code and leaves a callable passed by
  reference unattributed: the audit's § 8 probes show `untrack(lazyMemo)`,
  `runWithOwner(owner, lazyMemo)` and `createSignal(lazyMemo)` each compute a memo the
  call created. This ADR does not re-scope those rows; doing it through the dialect's
  own `callback_positions` for every dialect row of a `reads` walk is the follow-up, and
  would withdraw claims that today certify.
- **Read and refused, with the reason** (audit § 9): `solid-js` `createMemo` (the
  hydration gate is reached from any function argument unless `options.transparent`, and
  proving `options` absent needs a spread-aware fact this ADR does not add);
  `createComponent` and `createRoot` (each delegates through a wrapper -- `untrack`,
  `runWithOwner` -- whose argument slot differs from the delegating call's, and this
  ADR's delegates are checked against the delegating call's own arguments unmapped;
  `createRoot` also needs a `reads` row for `createOwner`); `createEffect`, `createStore` and the rest
  of the checkpoint's `dialect-silent` list, which nobody has read for `reads`.
- The rc.9 signals rows do not bind for another prerelease (rc.3, rc.6): rows are
  archive-scoped, and those archives carry no `reads` row for these primitives.
- `solid-js` `createSignal(0)` still needs its `creates` row to bind (scoped to the
  `browser` host), so under a host-free certification it stays open for that domain.
- A zero-argument `createSignal()` and `createSignal(...args)` are not proved
  primitives and refuse.

## Measured (2026-09-30)

Both instruments were run at the same base (`816d6f58`, ADR 0165), the release
binary, with and without this change (`make contract-coverage-census
TIER_HOSTS=` host-free, and `make primitives-checkpoint`), on a machine carrying
other runs; counts do not depend on wall time.

The census (the pinned 16-package corpus; every package is certified under
`solid-js` rc.0 and rc.9, and rc.0 is not an audited archive, so no row ever binds
there):

| Bucket (demanded sites) | Base | With the rows |
| --- | --- | --- |
| an operation is stated | 699 | 699 |
| determined: states nothing | 253 | 280 |
| degenerate: nothing determined | 200 | 173 |
| import finds open: some uses | 147 | 168 |
| import finds open: every import | 311 | 290 |

ADR 0165 took determined-nothing from 331 to 253; this recovers 27 of the 78 sites
it withdrew. `benchmarks/ecosystem/coverage-census.json` is re-pinned with it
(`closed-empty` 253 -> 280, `degenerate` 200 -> 173; all of the movement is
`@solid-primitives/rootless`, 0 -> 27 determined). The recovered `reads` closures, by
withheld-closure record at rc.9 (380 before, 370 after, none new):

- `@solid-primitives/memo`: `createMicrotask`, `createPureReaction`;
- `@solid-primitives/rootless`: `createHydratableSingletonRoot`, `createMicrotask`;
- `@solid-primitives/scheduled`: `scheduleIdle`, `throttle`;
- `@solid-primitives/storage`, `@solid-primitives/trigger` and
  `@solid-primitives/utils`: `createMicrotask`; `@solid-primitives/trigger`:
  `createTrigger`.

The primitives checkpoint (97 packages, 721 exports):

| Host | Clean, base | Clean, with the rows | partial / degenerate, base | partial / degenerate, with the rows |
| --- | --- | --- | --- | --- |
| none | 99 | 99 | 321 / 265 | 323 / 263 |
| browser | 99 | 99 | 326 / 260 | 328 / 258 |
| node | 100 | 100 | 321 / 261 | 323 / 259 |

No export became clean and none left clean. On every host two exports moved
`degenerate` -> `partial` (`@solid-primitives/rootless` `createHydratableSingletonRoot`,
`@solid-primitives/scheduled` `scheduleIdle`), and 28 more changed their causes and stay
`partial`: what keeps them from `clean` is another domain (`callbacks` and `returns` are
"never proposed" for most of them) or another wall, not `reads`.

What the census still refuses at rc.9 that names a dialect declaration: `solid-js`
`createSignal` 8 (all declined by name, `argument 0 is not a primitive by its
grammar`: `createSignal(initialValue, { ownedWrite, ...options })`,
`createSignal(value, { equals: false, ownedWrite: true })`,
`createSignal(untrack(timeout))`, and `createHydratableSignal`'s `serverValue`, each a
value that may be a function), `createRoot` 2 and `createMemo` 1. The larger walls the
census names for `reads` are not dialect rows: `property-access-unknown-accessor` (53
records), a veto that did not complete (29), `coercion` (27).
