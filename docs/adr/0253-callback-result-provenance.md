# ADR 0253: Callback-result provenance

- Status: accepted and implemented (2026-10-08).
- Owners:
  - the optional `call.callbackResults` catalogue (model, validation,
    digest family, certification refusal, wire, schema, authoring in
    `scripts/lib/callback-result-contracts.mjs`);
  - its projection and consumer in `contracts.rs`, `local_access.rs`,
    `execution_role.rs` and `interproc.rs`;
  - six specs.
- Fixture: `fixtures/reactive-ir/package-callback-result-consumer`.
- Relation: census-2 items B-results and C-results. Drafted in
  `rust/target/research/callback-results/`. Follows ADRs 0247 and 0249's
  pattern for a vocabulary addition.

## Context

A contract could say an export invokes a caller's callback, but not what the
package then does with the value that callback returns. Every
`OperationOutput` source reopened the callbacks domain, so a primitive that
reads, spreads, iterates, coerces or calls a callback's result could never
close it.

## Decision

1. **`callbackResults` lists, per producer callback, the package's uses of
   its result.**
   - Each use is an ordinary operation sourced from the producer's output,
     with its own protocol, timing, tracking, owner and count. A tracked
     callback's result may be used in the caller's untracked context, and the
     contract says so.
   - The census of uses can be unknown, partial, closed, or closed empty.
     `callableOnly` marks an optional call gated by an exact callable test.
   - Recursive producers, result-of-result traversal, wildcard paths and
     `HasInstance` are refused. Recursive traversal is a later item.
2. **Authored only:** generation never proposes it, certification refuses
   it, authoring needs citations, and probe results bind to the claim.
3. **Consumer.** A closed census lets a fresh primitive or getter-free literal
   result pass; a stated result call runs in its stated context. These remain
   `reactive-dispatch-unresolved` obligations:
   - dynamic values, proxies, getter-bearing objects and spreads;
   - unknown census;
   - escape;
   - aliased results;
   - wrappers;
   - project wrappers that did not prove the closure.
4. **Specs:** `createHydratableSignal`, `createPolled`, `createUndoHistory`,
   `createDerivedStaticStore`, `createAggregated`, and the non-recursive part
   of `resolveFirst`. All probe pairs pass in Chrome on rc.13, except one.
   `createPolled`'s constructor-read pair was dropped: Chrome did warn
   STRICT_READ_UNTRACKED, but from package code, because the read is the
   package's own constructor read (`timer/dist/index.js:124`, cited). The
   harness accepts only case-file attribution, so the read stays a cited claim
   without that pair.

## Consequences

- Primitives ledger, browser: unchanged at 91 of 112.
  - Five cases move from no proof to a proven misuse: `createHydratableSignal`,
    both `createPolled` cases, `resolveFirst`'s write case and
    `createDerivedStaticStore`.
  - Their correct twins keep obligations that need captures, lazy getters or
    recursive traversal.
  - No correct twin on any host has a violation.
- rc.13 sweep: no violation added or lost; uncertifiable +5.
