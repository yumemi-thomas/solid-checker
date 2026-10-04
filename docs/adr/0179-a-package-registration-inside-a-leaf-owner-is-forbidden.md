# ADR 0179: A package registration inside a leaf owner is forbidden

- Status: accepted and implemented (2026-10-05). Third lever of the owner's
  package-misuse goal of 2026-10-04.
- Owners: the consumer projection (`project_leaf_forbidden_operations`), the
  leaf-owner rule's synchronous-extent walk (`cleanup.rs`), and its wording
  (`leaf_contract_registration_wording` in the Solid 2 rules).
- Relation: extends the leaf-owner rules, which knew Solid's own primitives
  and exactly resolved in-project helpers, to package exports with an
  accepted contract. It reads the owner requirements ADR 0114 and ADR 0161
  publish, under a stricter condition. No wire change.

## Context

Solid 2's leaf owners (`createTrackedEffect`, an owner-backed `onSettled`)
forbid nested registrations. A cleanup there throws
`CLEANUP_IN_FORBIDDEN_SCOPE`, a primitive throws
`PRIMITIVE_IN_FORBIDDEN_SCOPE`, and either error halts reactivity
(`REACTIVITY_HALTED`). The misuse ledger has three such cases, all
runtime-detected: `createEventListener`, `createResizeObserver` and
`createTimer` called inside `createTrackedEffect`.

The accepted contracts already state what decides them. `createResizeObserver`
registers a computation on its caller's owner on every call (`compute`,
`min: 1`). The checker still reported only "the callback's exact synchronous
body cannot be resolved", because the leaf walk had no package arm.

## Decision

1. **A leaf-forbidden registration is a strict owner requirement.** The
   projection keeps an operation in `creates`, `cleanups` or `computations`
   only when all of these hold:
   - it imposes an owner requirement;
   - it takes the owner current at the call (`ambient-at-call`);
   - it is unguarded;
   - it is triggered by and runs at the call, on the same stack;
   - it is counted per call with `min >= 1`.

   This is stricter than ADR 0161's `guaranteed`, which reads the count
   alone: a leaf forbids a registration while it is current, so the
   registration must use the caller's owner within the call itself.
2. **The leaf walk asks the contract first.** A call in the leaf callback's
   own synchronous extent whose callee binds to such an export is one leaf
   operation per registration: a cleanup, or a computation. Each is a proven
   violation, anchored at the call. A nested function built in the callback
   is not walked, as before.
3. **The wording states the contract, not a runtime code.** "`listen()`
   registers a cleanup on its caller's owner on every call, and it is called
   inside `createTrackedEffect`, a leaf owner that forbids it; Solid throws
   here in dev." Which forbidden-scope error the runtime raises first depends
   on the export's internals, of which the contract states only the
   guaranteed registration. `createResizeObserver` throws on its
   `onCleanup` before it reaches its computation.

## Consequences

- A registration that may not happen (`min: 0`), one that waits for a later
  owner (`ambient-at-execution`), or one off the call's stack stays out. Such
  a call keeps the unresolved-callback obligation it had.
- `createTimer` is not decided: its cleanup is `min: 0`, the cleanup/effect
  disjunction ADR 0173 leaves open.

## Evidence

- Unit test
  `a_leaf_forbidden_registration_is_one_made_at_the_call_on_every_call`.
- Fixture `fixtures/reactive-ir/package-leaf-registration-consumer`:
  - three violations: a cleanup and a computation inside
    `createTrackedEffect`, and a cleanup inside `onSettled`;
  - no violation for a may-register export, a silent export, a call deferred
    with `queueMicrotask` (which is instead the existing unowned-call
    violation), or calls in a component body.
- Coverage: 162 fixtures, 838 findings. Only the new fixture's 7 are added.
- Misuse ledger, release binary over the ADR 0178 tier (no regeneration:
  the change is consumer-side):
  - 66 -> 68 static violations of 123: `createEventListener` and
    `createResizeObserver` inside `createTrackedEffect`;
  - both runtime-detected (`PRIMITIVE_IN_FORBIDDEN_SCOPE` and
    `CLEANUP_IN_FORBIDDEN_SCOPE`, each followed by `REACTIVITY_HALTED`);
  - no correct twin is flagged.
