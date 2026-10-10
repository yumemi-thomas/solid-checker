# ADR 0252: Owner guards are decided from exact argument facts

- Status: accepted and implemented (2026-10-08).
- Owners:
  - `ArgumentLiteralFact` (`solid-facts/src/ast/mod.rs`; AST facts schema
    51 to 52);
  - its adapter into the guard evaluator
    (`contract_semantics/consumer.rs`);
  - `owner_requirement_at_call` and `push_owner_requirement`
    (`solid-reactive-ir/src/owners.rs`);
  - four specs.
- Fixture: `fixtures/reactive-ir/package-owner-guards-consumer`.
- Relation: census-2 items C-guards and A-guards
  (`rust/target/research/census-2/CENSUS.md`). Drafted in
  `rust/target/research/guards/`. Extends ADR 0223's per-call guard
  evaluation to literal, property and length guards.

## Context

An owner or leaf registration can be guarded. `createShortcut` registers
listeners only for a non-empty key list, and `createEventListenerMap`
registers an effect per handler entry. The owner projection evaluated only
bare value-kind guards, so every other guard was unknown at a call. The
registration therefore stayed possible (uncertifiable, ADR 0231) even when
the call's literal arguments proved it.

## Decision

1. **Facts record a normalized literal per argument.** The kinds are:
   null, boolean, a bounded non-negative integer, a cooked string, a
   function, an object's final own data properties, or an array's exact
   length. Transparent TypeScript wrappers are peeled. Spreads, accessors,
   computed keys and `__proto__` make the value unknown. An argument at or
   after an argument spread has no exact slot. No Oxc node crosses the facts
   seam.
2. **The owner projection uses the existing guard evaluator.**
   - A true guard keeps the operation's lower bound.
   - A false guard removes the branch.
   - An unknown guard leaves a possible requirement.
   - A proven guard never raises a `min: 0` operation.
3. **A certain registration wins over a possible one at the same call.**
   `push_owner_requirement` kept the first requirement of an operation at a
   call, and the optional unguarded one sorted first. A certain one now makes
   the merged requirement certain; the context is the call's either way.
4. **Claims:**
   - `createEventListenerMap` adds a `resize` handler guard;
   - `createShortcut` adds a two-key guard;
   - `createPageLeaveBlocker` is unchanged in substance, now decided at the
     call.

   All probe pairs pass in Chrome on rc.13. The event-listener `3.0.0-next.5`
   claim moves to its own pair directory, so the `_pairs` files that
   `3.0.0-next.3` shares stay untouched.
5. **`createPointerListeners` stays optional.** Its parser lower-cases every
   config key (`pointer/dist/helpers.js:19-24`), so a later `ondown:
   undefined` overwrites `onDown`. A callable `onDown` alone proves nothing.

## Consequences

- Primitives ledger, browser: 91 of 112 report correctly (was 88):
  `createEventListenerMap`, `createShortcut` and `createPageLeaveBlocker`.
  No correct twin on any host has a violation.
- rc.13 sweep: no violation added or lost.
- An edit to a call argument's literal is no longer a same-shape edit: owner
  guards read it, so the incremental engine recomputes reachability and owners
  for it. The session test now uses a same-length JSX text edit.
