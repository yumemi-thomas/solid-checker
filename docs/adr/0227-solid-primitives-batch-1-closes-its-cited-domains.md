# ADR 0227: Solid Primitives batch 1 closes its cited domains

- Status: accepted and implemented (2026-10-07).
- Owners: `pkg/contracts/authored/specs/@solid-primitives+*` (11 version
  specs) and their shared pairs under `specs/_pairs/@solid-primitives+*`;
  `scripts/author-contracts.mjs`; `contract_semantics/validate.rs` (the
  `compute` rule).
- Relation: applies ADR 0226 (cited closures, premises P1 and P2,
  timing-aware reads, optional-owner leaf rules) to the batch of Solid
  Primitives versions ADR 0223 shipped positively. It is the first step of
  the owner's focus on primitives: every corpus call site of a primitive
  becomes a proven violation or certified clean.

## Decision

1. **Complete specs replace the ADR 0223 positive-only ones.** They cover
   `event-listener` 3.0.0-next.3 and next.5, `resize-observer`, `keyboard`
   2.0.0-next.5, `utils`, `media`, `scroll`, `memo`, `refs`, `raf` and
   `timer`.
   - Each closed domain carries a citation of the installed source, or names
     its premise.
   - Each positive claim has a probe pair, and all 70 pairs pass in Chrome on
     rc.13.
2. **Pure functions need no pair.** That covers `noop`, `trueFn`, `clamp` and
   `keys`. A claim with no rule ships on its closure citations alone, if its
   only operations are a bare return of a non-reactive value or a possible
   (minimum 0) ambient coercion of the caller's value. Neither can yield a
   violation.
3. **The tool refuses what the decoder would.** An open domain with an empty
   list is now an authoring error. Two such documents had made every corpus
   project fail to load.
4. **A `compute` may tolerate a missing owner.** ADR 0114 required it to
   require one. rc.13's computed nodes run unowned and throw only under a
   leaf owner, so a `compute` now requires child owners of an owner it does
   not create, whose presence it may require or leave unconstrained but never
   forbid. ADR 0226's leaf rule reads that child requirement.
5. **Dropped, because no diagnostic can show them, rather than shipped
   unprobed:**
   - `makeEventListener`'s returned removal function;
   - the hydration-only `onSettled` registration in `createMediaQuery` and
     `createWindowSize`, whose `creates` stays open;
   - `entries`, which returns caller values.

## Consequences

- 14 of 35 export rows are closed on all four domains `SC9005` demands. The
  rest stay open for stateful returned functions (`defer`, `targetFPS`,
  `makeTimer`'s clear function, `createMemoCache`), lazy size getters, and
  `access`/`asArray` return unions.
- rc.13 corpus: no violation moved. Obligation sites attached to these ten
  packages went from 21 to 17, and uncertifiable rows from 3,429 to 3,424.
  The corpus uses these primitives lightly, and mostly correctly.
- Twins: unchanged at 25 proven and 13 uncertifiable, out of 38. No correct
  twin gains a finding.
- About 30 primitive packages in the corpus still have no contract.
