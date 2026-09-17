# ADR 0111: The package-contract word belongs to the dialect

- Status: accepted (2026-09-17)
- Date: 2026-09-17
- Owners: the `Dialect` vocabulary seam, contract emission in
  `solid-reactive-ir::interproc`, and `docs/adding-a-dialect.md`
- Relation: applies the rule `rust/ARCHITECTURE.md` already states ("shared
  code asks the selected vocabulary and receives that dialect's answer") to the
  one layer that did not follow it. Does not change ADR 0110; it closes a gap
  the 1.x retirement made visible.

## Context

`rust/ARCHITECTURE.md` describes three dialect seams — vocabulary, compiler,
rules — and AGENTS.md forbids putting dialect-specific behaviour in shared code
when the seam can express it. The Solid 1.x retirement was a fair test of that
claim, and the vocabulary, compiler and rules seams passed: a whole dialect came
out by deleting its crate and its vocabulary file.

Contract emission did not. `primitive_callback_execution`
(`solid-reactive-ir/src/interproc.rs`) answered the *package-contract* word —
`inline`, `deferred`, `tracked` — from a hardcoded `match` on `Primitive`. It
delegated to the dialect for three primitives and stated the rest itself:

```rust
(P::CreateMemo | P::CreateSignal | … , 0) => Some("tracked"),
(P::OnSettled | P::Action | P::CreateReaction | P::OnCleanup, 0) => Some("deferred"),
(P::CreateRoot | P::Untrack | P::Flush, 0) | (P::RunWithOwner, 1) => Some("inline"),
```

Three facts made this worth fixing rather than noting:

1. **Three of its arms were already dead.** `createResource`, `on` and
   `mergeProps` are Solid 1.x names; `Solid2` carries none of them. Shared code
   held answers no dialect in the build could reach.
2. **It produced a real defect.** The direct-invocation rung read this table and
   the chain rung read the dialect's `tracked_callback_timing`; they disagreed,
   and every tracked callback published `queued` where four 2.0 primitives run
   during the creating call. Fixed on 2026-09-17, but the disagreement was
   structural — two sources of truth for one question.
3. **The forward checklist did not warn about it.** `docs/adding-a-dialect.md`
   lists what a new dialect owns and what stays centralized, and this table
   appeared in neither. A third dialect would have been written, shipped, and
   only then found that it could not state a contract word of its own.

## Decision

The contract word is dialect-owned, through
`Dialect::contract_callback_execution_at(primitive, argument, argument_count)`.
It defaults to `None` — a dialect that states nothing yields "unknown", which
contract emission already handles — and `Solid2` states 2.0's answers.

`interproc.rs` keeps only the projection from `Execution` to the wire word.

**It is a separate method from `callback_execution_at`, deliberately.** That one
answers attribution for the checker's own analysis — whose reads a callback
subscribes. This one answers what a published contract promises about observable
scheduling relative to the exported call. They agree for most primitives and
deliberately diverge for some: 1.x's `createResource` fetcher is `Deferred` for
attribution and `Inline` for a contract; `onCleanup` carries no
`callback_executions` row in 2.0 at all and still promises `deferred`. Deriving
one from the other would lose exactly those cases.

## Consequences

- No contract changed. The corpus is byte-identical across the move: 97
  fixtures, 328 declined closures, 147 artifact cases, 174 operations, 1259
  proof candidates, 5023 open claims before and after.
- `the_contract_words_are_stated_by_this_dialect_not_by_shared_code` pins the
  answers where they now live, including the `onCleanup` case that proves the
  word is stated rather than derived.
- The three 1.x-only arms are gone rather than ported. A future dialect that
  carries `on` or `mergeProps` states its own answer; the reasoning for 1.x's is
  in this ADR and in the git history of `interproc.rs`.
- `docs/adding-a-dialect.md` names the method, so the next dialect meets it on
  the checklist instead of in a defect.

## What this does not claim

The seam is now correct for *callback execution*. It is not a survey. This ADR
was written after finding one violation while removing a dialect; the honest
statement is that shared code still holds primitive-keyed logic elsewhere
(`owners.rs`, `source_discovery.rs`, `static_api.rs`, `cleanup.rs`), and
whether each is dispatch or version-specific behaviour has not been audited.
That audit is the natural next piece of work, and it should happen before a
third dialect rather than during one.
