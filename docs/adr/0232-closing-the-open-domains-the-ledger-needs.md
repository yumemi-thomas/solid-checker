# ADR 0232: Closing the open domains the primitives ledger needs

- Status: accepted and implemented (2026-10-08).
- Owners: the contract parameter-member read in
  `solid-reactive-ir/src/local_access.rs`; the `access` spec
  (`@solid-primitives+utils@7.0.0-next.4`) and the `createPointerPosition`
  spec (`@solid-primitives+pointer@1.0.0-next.2`).
- Relation: follows ADR 0226 and ADR 0227. Revisits ADR 0227's note that
  `access` keeps its return union open.

## Context

17 browser cases in the primitives ledger prove their misuse, but their
correct twin is not certified clean: the contract leaves a domain open, so
the twin keeps a `package-contract-incomplete` notice. A source review of each
found that 14 cannot close under the current contract format: lazy getters,
mutable returned APIs (`createEventStack`, `createDate`), hidden singleton
cleanups (`useKeyDownList`, `createKeyHold`), recursive returned callbacks
(`resolveFirst`). Three could move.

## Decision

1. **An absent argument has no member to read.** A contract's
   parameter-member read at a position the call leaves empty, with no spread
   that could fill it, now yields nothing. It used to be a
   `reactive-dispatch-unresolved` obligation (`createLazyMemo` called without
   options).
2. **`access` closes `returns`.** Its source is one line
   (`utils/dist/index.js:88`): return `v`, or `v()` when `v` is a zero-arity
   function. The two existing return operations name exactly those
   alternatives, so the union is complete. Neither alternative is claimed to
   be non-reactive.
3. **`createPointerPosition` closes `reads` and `creates`.** Five new
   operations (the state signal, the listener effects, the tracked target, the
   callable initial state and its tracking) each have a probe pair, and all
   six pairs pass in Chrome on rc.13. The return now names the produced state
   as a reactive accessor.

## Consequences

- Primitives ledger, browser: 18 cases report correctly (was 16):
  `utils-access-argument-read` and `pointer-createPointerPosition-top-level-read`.
  No correct twin on any host has a violation.
- Coverage: unchanged (198 fixtures).
- `createLazyMemo`'s correct twin is still not clean: its calc callback is
  tracked and resource-triggered with no stated point of execution, so a read
  inside it stays a `strict-read-untracked` obligation.
