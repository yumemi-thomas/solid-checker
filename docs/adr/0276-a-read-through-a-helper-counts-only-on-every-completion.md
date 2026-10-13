# ADR 0276: A read through a helper counts only on every completion

- Status: accepted and implemented (2026-10-13).
- Owners:
  - `solid-facts` `AstFacts::completion_sites` (AST facts schema 59);
  - `solid-reactive-ir/src/interproc.rs`: `site_on_every_completion`, the
    `SummaryRead::local`/`conditional` flags, guaranteed graph edges and the
    `read_is_direct` gate;
  - `propagate_summary_deltas` in `solid-reactive-ir/src/lib.rs`.
- The read-channel counterpart of ADR 0275. Narrows the
  statically-present-code premise (2026-10-13) at a call boundary, at the
  owner's request (2026-10-13). No contract or receipt change.

## Context

ADR 0201 proves a strict read attributed through a called function's summary
when the read runs while the call does, and ADR 0204 extends that through
directly entered helpers. "Runs while the call does" was positional: a read
written directly in the body counted wherever it was written.
`readsWhen(5)` with `if (typeof value === "number") return value; return
count();` was a violation, although the call returns before reading. The
`MaybeAccessor` shape, `x ?? y()`, `a || b()` and reads after an early
`return` are common in helpers and in packages.

## Decision

1. **A completion site is syntax.** `AstFacts::completion_sites` lists the
   call, `new` and member expressions written in their innermost function's
   straight-line flow, and in no conditional, logical right operand, logical
   assignment (`??=`, `||=`, `&&=`), loop, switch or try. An earlier
   `return`, `break`, `continue` or `throw` of that function must not be able
   to skip them. A thrown guard counts because a caller may catch it and
   carry on. In an optional chain, what precedes the first `?.` is a site and
   nothing after it is. A `break` or `continue` is read as a possible exit,
   which only proves less.
2. **A summary row carries its completion standing.** `local` says the row
   is not proven on every completion of the function whose summary holds it.
   For a row found in that body, that means its origin is not a completion
   site, or `body_site_runs_during_call` withholds it. `conditional` says a
   hop below was not proven. Crossing a call edge folds `local` into
   `conditional`. The new `local` is whether any call of that edge is a
   completion site of the caller. Graph edges and transported provenance
   edges record that; composite dispatch edges and callback forwardings do
   not, so they count as unproven.
3. **Only a row with neither flag is direct.** `read_is_direct` refuses it
   first, and an activated default (ADR 0204) is direct only when its own
   read is a completion site of the default. A refused row keeps the existing
   `summary_attributed` projection: uncertifiable, with wording that names
   "only on some of its paths".
4. **Deduplication keeps the better-proven row.** A row is dropped only when
   an equal row is at least as proven. A weaker one never hides a stronger
   one that arrives later in the fixed point.
5. **The call in the component stays under the premise.** Only what the
   called function and its callees do is decided here.

## Consequences

- rc.13 corpus (browser host): 573 → 564 violations, 3,719 → 3,729
  uncertifiable, nothing added. Every one of the nine moved reads was checked
  by hand to be on some paths only. Two are in openbot: `saving()` as the
  right operand of `||`, and `dirty()` inside conditions. Seven are in
  probus-hk: reads after early returns in `centreOn` and `endOf` (four sites),
  and `ownExpanded()` as the right operand of `??`. The first version excluded
  whole optional chains and also moved two error-menu-web reads
  (`account.user()?.user_id`). The chain rule in (1) restores them.
- Sweep wall time is unchanged (86 s → 87 s summed over 49 projects).
- Fixtures: `accessor-provenance` `DbBody`'s `initial()` (right of `??`) and
  `async-call-prefix` `SuspensionFreeLoop` (a read in a loop body) become
  uncertifiable. Loop bounds are not modelled.
- Not yet:
  - argument sensitivity, which would decide `typeof value === "number"`
    from the call;
  - completion standing for a parameter the helper invokes conditionally
    (`if (x) cb()`), whose callback reads keep their existing proof;
  - composite dispatch and callback-forwarding edges, which count as
    unproven;
  - async reads (SC5001) through helpers, which do not consult the flags.

## Evidence

- Review round 1
  (`../solid-checker-rc13-corpus/experiments/review-0276/REVIEW-1.md`)
  found two false violations in this change: a logical assignment's right
  side recorded as a completion site, and a thrown guard that a catching
  caller survives. Both are now excluded, with no corpus change (564/3,729).
  It also confirmed three pre-existing false-violation paths, which are not
  fixed here:
  - an accessor passed by reference to a helper that calls it
    conditionally;
  - an inline arrow passed to a helper that calls it conditionally, with an
    SC5001 variant;
  - a class instance-field initializer in a component or leaf body.

- Fixture `reactive-ir/summary-completion-read` (nine cases, `tsc --noEmit`
  clean).
- Unit tests `completion_sites_stop_at_a_possible_exit` (solid-facts) and
  `summary_edges_carry_completion_standing` (reactive IR).
