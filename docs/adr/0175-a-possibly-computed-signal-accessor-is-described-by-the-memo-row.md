# ADR 0175: A possibly-computed signal accessor is described by the memo row

- Status: accepted and implemented (2026-10-04). The owner chose this wall on
  2026-10-04 ("createSignal undecided argument").
- Owners: the dialect (`Solid2::computed_accessor_read`,
  `computed_accessor_read_archive`), the generator's reading walk
  (`returns_walk::returned_identifier_read`), and the described-callable census
  (`owned_returned_accessor_witness`, unchanged in code).
- Relation: amends ADR 0162 § 1 (the two rows "never overlap") and its archive
  binding ("`solid-js`' own hydration/server factories remain refused"). It
  rests on the audit
  [`2026-10-04-solid-2-rc9-returned-memo-and-signal-accessors.md`](../package-contract-v2/audits/2026-10-04-solid-2-rc9-returned-memo-and-signal-accessors.md).
  No new value shape, read or wire field.

## Context

The `@solid-primitives` misuse ledger has 36 strict-read cases the runtime
detects (`STRICT_READ_UNTRACKED`) and the checker does not. In 26 of them the
export's contract states no `returns` at all, so a caller reading what it
returns at a component's top level cannot be proven. The largest refusal class
is ADR 0146's argument premise: the accessor comes from a `createSignal` whose
first argument grammar cannot classify. Examples:

- `createTween`: `createSignal(target())`;
- `createPointerPosition`: `createSignal(config.value ?? DEFAULT_STATE)`;
- `createWSState`: `createSignal(ws.readyState, { ownedWrite: true })`;
- `createDate`, `createReducer` and `createPolled`.

The premise is right for the claim it guards. `createSignal(fn)` takes the
writable-memo path, so the read is not inert. But it is not the only claim
available. On both paths the accessor's read observes the node's value,
re-runs the computation the creating call registered (only on the memo path),
or throws. That is ADR 0162's computed-accessor row exactly. The consumer
projects either read to the same `accessor` leaf, which is all a strict-read
finding needs.

A second, smaller class failed on the archive binding. `refs` `resolveFirst`,
`signal-builders` `capitalize`/`ceil` and `pagination` `createPagination`
import `createMemo` from `solid-js`, whose declaration is `solid-js`' own, and
the row was audited for `@solidjs/signals` only.

## Decision

1. **`createSignal`'s tuple slot 0 joins the computed row.** The two rows now
   overlap there. `inert_accessor_read` is the stronger answer and is asked
   first. `computed_accessor_read` is what remains when its argument condition
   cannot be discharged. The computation the memo path registers is
   `createSignal`'s callback position 0 (`callback_execution_at(CreateSignal, 0)
   == Tracked`), so it is accounted at the creating call, as ADR 0162 requires.
   The census's argument condition for the row is unchanged
   (`memo_arguments_discharged`):
   - the computation in an undisplaced, non-spread slot;
   - every later slot a primitive, or at the dialect's options position a
     callback-free options literal.

   The memo path keeps `equals`, so an options binding stays refused.
2. **`solid-js@2.0.0-rc.9` is audited for both rows.** Its three client builds
   hand back the `@solidjs/signals` call's own result on every branch,
   hydration included. Its three server builds hand back a read that re-runs
   the registered computation or throws, or a closure over a non-function
   value. The `reads` rows of these two factories stay refused: the hydrated
   computation's own reads are the creating call's and were not walked.
3. **The generator proposes the read the binding makes possible.** The census
   accepts only the exact described call the proposal enumerates, so the
   reading walk now chooses the read for a returned identifier from its one
   binding in the function's own body:
   - `const [x] = call(first, …)` with `first` not proven non-callable by
     grammar proposes `owned-memo`. Unary operators count as non-callable.
   - a whole call result, `const x = call(…)`, proposes `owned-memo`.
   - anything else keeps `owned-signal`: no binding, two bindings of the
     spelling, or a non-call initializer.

   The spelling only proposes; the census decides from the producer's trace.

## Consequences

- A weaker claim is never published where the stronger one holds: the census
  asks the inert row first, and a proposal that names the other read is
  withdrawn by name.
- `createOptimistic` proposes `owned-memo` by the same binding rule. The census
  refuses it, as it refused `owned-signal`, because no row covers
  `createOptimistic`.
- Structural returns (ADR 0172) take the same witness for an accessor leaf, so
  a `[state, dispatch]` tuple over such a signal certifies its leaf too.

## Evidence

- `solid-dialect` tests: `only_the_memo_and_signal_accessor_reads_are_computed`
  and `the_computed_accessor_read_binds_only_its_audited_archive`.
- Census tests: `a_memo_is_witnessed_only_for_a_computed_audited_accessor`
  covers:
  - the undecided `createSignal` read;
  - its options and spread refusals;
  - the setter slot;
  - `solid-js` rc.9 accepted and signals rc.3 refused.
- Generator test:
  `a_reading_callable_is_proposed_for_returned_literals_and_identifiers`.
- Corpus: `implementation-census-owned-signal-reads` (`createFrom`) and
  `callback-slot-derived-store` now propose `owned-memo` for a `createSignal`
  over a parameter.
- Measurement, browser tier regenerated with `--carry`:
  - 39 exports gain an `owned-memo` `returns` claim (closed for 20 of them)
    and none loses one; `signal-builders` accounts for 33.
  - The misuse ledger's static violations go from 53 to 60 of 123, every new
    one runtime-detected (`STRICT_READ_UNTRACKED`): `createTween`,
    `createPointerPosition`, `createWSState`, `createReducedMotion`,
    `resolveFirst`, and `signal-builders` `capitalize` and `ceil`.
  - No correct twin is flagged.
  - A first run without the zero-argument rule proposed `owned-memo` for
    `createSignal()` and lost `createEventSignal`'s certified read. The
    generator now keeps `owned-signal` there, which is what the census
    discharges.
