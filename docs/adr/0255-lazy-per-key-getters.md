# ADR 0255: Lazy per-key getters

- Status: accepted and implemented (2026-10-08).
- Owners:
  - `ValueShape::LazyGetterObject` (wire kind `lazy-getter-object`) with its
    validation, digest family, certification refusal and schema;
  - the Get consumer in `local_access.rs`;
  - `scripts/lib/lazy-getter-contracts.mjs`;
  - the `resize-observer` (`createElementSize`) and `static-store`
    (`createStaticStore`) specs.
- Fixture: `fixtures/reactive-ir/package-lazy-getter-consumer`.
- Relation: census-2 items B-lazy, C-lazy and B-observer. Drafted in
  `rust/target/research/lazy-getters/`.

## Context

`createStaticStore` (and `createElementSize`, built on it) returns an object
whose getters are lazy:
- a Get with no observer returns a plain value;
- a Get under an observer creates and caches a per-key signal, which later
  Gets read (`static-store/dist/index.js:27-54`).

ADR 0251 had to describe `createElementSize`'s members as `unknown`, so each
of its 30 member reads in the rc.13 corpus was a
`reactive-dispatch-unresolved` obligation.

## Decision

1. **A returned object may be a `lazy-getter-object`.** It is a finite key
   list or argument-derived keys. It is authored only and refused by
   certification.
2. **The conservative reading.**
   - A Get directly in tracked code (JSX, a memo, an effect's compute) is
     clean.
   - Any other Get is an uncertifiable obligation, even where a person could
     see whether the key was primed. Proving that would need cache-priming
     dominance, which is not modelled.
   - No lazy read becomes a proven violation.
3. **Specs.** `createElementSize` uses it. So does `createStaticStore`, with
   `callbacks` left open, because spreading the caller's initial object can
   run the caller's getters (the authoring rule for property Gets). The draft
   closed `callbacks` there, and the authoring tool refused it.
   `createWindowSize` is left with `returns` open: enumerating it replaced
   about a dozen import notices with 53 member-read obligations in the corpus
   (mostly component-prop reads), for no ledger gain.

## Consequences

- rc.13 corpus:
  - `createElementSize` member-read obligations drop from 30 to 16;
  - uncertifiable drops from 3504 to 3491;
  - no violation is added or lost.
- Primitives ledger unchanged at 92 of 112. No correct twin has a violation.
