# ADR 0183: A package's owned-computation callback is an owned scope

- Status: accepted and implemented (2026-10-05). Seventh lever of the owner's
  package-misuse goal of 2026-10-04.
- Owners:
  - the dialect's eager owned-computation slots
    (`Dialect::eager_owned_computation_slot`);
  - the generator's callback rows (`interproc.rs`) and their emission
    (`inferred_contract.rs`);
  - the certification census (`require_owned_computation_callback` and its
    two callers, and the owner resource's withholding);
  - the consumer projection (`project_guaranteed_callback_parameters`) and the
    execution role (`contract_owned_computation_callback_role`).
- Relation: reads the `owner: created` vocabulary ADR 0114 introduced for
  callbacks, which the generator never published for a tracked row until now.
  No wire change.

## Context

A signal write inside a memo or effect compute throws
`REACTIVE_WRITE_IN_OWNED_SCOPE` in dev. The checker reports it for Solid's own
`createMemo`. A package that hands its caller's function to `createMemo`
(`resolveFirst`, `createDerivedStaticStore`) went unreported. Its contract said
the callback runs tracked, but not that it runs under an owner, nor that it
runs at all. The misuse ledger has five such runtime-detected cases, each
`contract-missing`.

## Decision

1. **The dialect names the eager owned-computation slots.** For Solid 2 these
   are `createMemo(fn)` with one argument and `createEffect(compute, effect)`'s
   compute with two. Source: `@solidjs/signals@2.0.0-rc.9`. A memo computes
   eagerly unless `lazy` is set. An effect's compute runs during the call,
   even with `defer`. Calls carrying options are not stated.
2. **The generator states the owner and the lower bound.** A parameter that is
   exactly such a slot's argument, with no enclosing callback position, gets a
   tracked row with `owner: created`. Where the call covers every normal
   completion of the export's body, the row's count is `min: 1`.
3. **The census proves both from the producer's facts.**
   - The invoke must be the caller's value as the exact argument of the slot,
     in a direct, uncaptured `solid-js` call of the export's own body.
   - The owner must be the generator's children-capable created owner.
   - `min: 1` must rest on a call the producer states reachable and
     unconditional.
   - The owner resource is answered through its one producing operation.
   - The document closes neither the owner's `productions` nor the resource's
     `states`, so both read back open. The census compares their items, not
     their closure.
   - Every refusal here carries one marker. The certifier then withdraws only
     the created owner, its resource and the `min: 1` bound
     (`ExportSemantics::weaken_created_owner`). The tracked row stays, as it
     was certified before this ADR. Withdrawing the whole operation left its
     resource behind, and that refused 21 checkpoint packages outright.
4. **The consumer reads a guaranteed slot as a tracked callback.** The
   accepted operation must be tracked, at the call on the same stack,
   unguarded, under a children-capable created owner, with `min >= 1`. A
   function literal written as that whole argument then takes the tracked
   callback's role. A write directly in it is `SC2001`, wherever the export
   is called. This holds even when the `callbacks` enumeration is open: the
   claim is per item.
5. **The completion cover counts a call as a whole initializer or return.**
   `const x = a()` and `return a()` run `a()` on that path, as `a();` does.
   Once every live path has run a candidate, the walk stops: later statements,
   such as a `for…in` loop it does not model, cannot undo the cover. This also
   serves ADR 0173's owner registrations, on both the generator and the census
   side.

## Consequences

The following stay unreported:

- an export that only may run the callback;
- a created leaf owner (writes are legal there);
- a callback inside a compute the package writes itself (until the amendment
  below);
- a function passed by name;
- a closure the compute only returns.

## Evidence

- Census tests:
  - `an_owned_computation_callback_is_an_eager_slot_of_a_direct_solid_call`:
    `createMemo(fn)` and an effect compute are accepted. Refused: a memo with
    options, an effect function, a conditional call under the strict floor,
    another module's `createMemo`, a captured call, and `createTrackedEffect`.
  - `an_emitted_owned_computation_row_reads_back_as_the_census_expects`:
    the corpus fixture's emitted row and resource pass both checks.
- Model test `weakening_a_created_owner_keeps_the_invoke_and_drops_an_unnamed_resource`.
- Cover tests `completion_cover_counts_a_whole_initializer_or_returned_call`.
- Corpus fixture `owned-computation-callbacks` pins the generator. Three
  existing fixtures gain the created owner on their `createMemo` callback rows:
  `implementation-census-memo-accessors`, and `callback-slot-derived-store`
  with its server twin. `createLive` and `derive` also gain `min: 1`.
- Consumer fixture `package-owned-computation-consumer`: two `SC2001`
  violations, and every negative stays silent or uncertifiable.
- Browser tier regenerated with `--carry`:
  - the misuse ledger goes from 74 to 77 static violations of 123:
    `resolveFirst`, `createDerivedStaticStore` and `createBodyCursor`, each
    runtime-detected (`REACTIVE_WRITE_IN_OWNED_SCOPE`);
  - no correct twin is flagged, and no checkpoint row changes status;
  - every tier difference is a gained created owner, most with `min: 1`.
    Beyond the three, `eventListener` (`createEffect(props, …)`),
    `resolveTokens` and `createPageLeaveBlocker` gain one. No operation or
    closure is lost.
- Coverage moves only the new fixture. The contract corpus moves only the
  three fixtures above, plus the new one. The 38-app sweep is unchanged
  (264 violations, `ADDED 0` in node and browser mode), because the tier
  still matches no installed environment there.
- `until` stays missed: its memo is created inside `createRoot`, which is an
  enclosing callback position. `capitalize` stays missed too. It is
  `createMemo(() => … string() …)`: the caller's function is called inside a
  compute the package writes itself (the fixture's `deriveWrapped`), so the
  chain has an enclosing wrapper and no owner is stated. Composing an owned
  computation through that wrapper is the next step for this rule.

## Amendment: the wrapped form (2026-10-05)

`capitalize` is `(string) => createMemo(() => { const s = string(); … })`. The
caller's function is not the slot's argument; it is called inside the compute
the package writes, which is the slot's argument. It runs under that memo all
the same.

- **Generator.** A `tracked` row whose one enclosing wrapper is an eager owned
  slot gets `owner: created` when:
  - the slot's whole argument is the synchronous literal the parameter call
    sits directly in;
  - the call covers that literal's body (`wrapped_owned_computation_call`).

  `min: 1` follows when the memo call also covers the export's body.
- **Census.** The parameter call must be stated:
  - `captured`;
  - `calleeUnwrittenParameter` = the row's parameter;
  - `unconditional`, which the producer states only for a plain synchronous
    flow owner. So an async compute, whose call after an `await` runs with no
    owner, is refused without a new fact.

  Its `enclosingCallable` must be, by identity, the one callable the eager
  slot's `argumentCallables` carries, in a direct `solid-js` call the floor
  admits.
- **Cover.** An expression-bodied arrow (`fn => createMemo(…)`) is read as its
  one expression statement.

Evidence:

- Census test
  `a_wrapped_owned_computation_callback_is_an_unconditional_call_in_the_slot_literal`.
  A conditional call and a literal the slot does not carry by identity are
  refused.
- Cover test `completion_cover_reads_an_expression_bodied_arrow_as_its_one_statement`.
- Corpus:
  - `owned-computation-callbacks` adds `deriveArrow` (owned, `min: 1`), plus
    `deriveWrappedMaybe` and `deriveWrappedAsync` (no owner). `deriveWrapped`
    becomes owned with `min: 1`.
  - Rows gain the owner in `callback-deferred-untracked-chain`,
    `callback-untracked-wrapper`, `multi-role-callback-parameter` and
    `implementation-census-memo-accessors`. Each is a parameter called
    unconditionally inside an effect or memo compute the export writes.
- Browser tier regenerated with `--carry`:
  - the misuse ledger goes from 77 to 78: `capitalize`, runtime-detected;
  - no correct twin is flagged, and no checkpoint row changes status;
  - the only tier difference is `capitalize`'s created owner.
