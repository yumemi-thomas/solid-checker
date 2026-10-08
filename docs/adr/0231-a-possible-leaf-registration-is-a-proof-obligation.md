# ADR 0231: A possible package registration in a leaf owner is a proof obligation

- Status: accepted and implemented (2026-10-08).
- Owners: `project_leaf_forbidden_operations` (`solid-reactive-ir/src/contracts.rs`),
  the contract branch of the leaf scan (`cleanup.rs`),
  `LeafOwnerOperation::possible`, and the leaf wording in
  `solid-v2/rules/src/lib.rs`.
- Relation: amends ADR 0179, which reported only registrations made on every
  call. Follows ADR 0161's treatment of `min: 0` owner requirements.

## Context

ADR 0179 projects, from an accepted contract, the owner registrations an
export makes at the call. It kept only those with `min >= 1`. A registration
stated with `min: 0` was dropped, so a call to that export inside a leaf owner
fell through to the in-project helper walk, which cannot see into a package.
The checker then reported `reactive-dispatch-unresolved`, a finding about an
unresolvable callback, which was wrong. The same happened to a guarded
registration whose guard the syntax does not settle.

`createShortcut` is the measured case. An empty key list returns before it
registers its listeners, so its cleanups are `min: 0`. Inside
`createTrackedEffect`, Chrome raises `CLEANUP_IN_FORBIDDEN_SCOPE`.

## Decision

1. The projection keeps a `min: 0` registration, marked unguaranteed. A
   guaranteed registration of the same kind and guard covers it.
2. The leaf scan skips a registration only where its guard is false at the
   call. A guaranteed one is a violation, as before. One that may happen
   (`min: 0`, or a guard the call does not settle) is a
   `leaf-owner-forbidden-call` with `possible` set, projected as uncertifiable.
3. Its wording says the export *may* register, on some calls, so where it
   happens Solid throws.

## Consequences

- Coverage: one finding moves. In `package-leaf-registration-consumer`,
  `startTicker()` inside `createTrackedEffect` goes from
  `reactive-dispatch-unresolved` to `leaf-owner-forbidden-call`
  (uncertifiable).
- Primitives ledger: five case-host pairs move from an unrelated finding to the
  right rule, uncertifiable. They are `createShortcut`'s leaf-owner case on
  browser and none, and the leaf-owner cases of `createResizeObserver`,
  `createEventListener` and `createTimer` on none, where the host-free claim
  lets the registration be skipped. No correct twin gains a finding.
- `createShortcut` stays short of a violation: the contract format has no
  guard for "a non-empty array", which is what would make its cleanups
  `min: 1` where the key list is literal and non-empty.
