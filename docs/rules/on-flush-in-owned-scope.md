# on-flush-in-owned-scope

`SC3002` · **warning** · violation

`onFlush` is called in a component body or a computation.

## What it does

`onFlush` keeps a write out of another update's hold, so it belongs in an event
handler or an effect's apply. rc.21 warns `ON_FLUSH_IN_OWNED_SCOPE` when it runs
under an owner that is no leaf (`onFlush`, `@solidjs/signals@2.0.0-rc.21`
`dist/dev.js`). The call is placed by the owned-scope write role: a component body,
a compute and a root body are reported, and an effect's apply, `onSettled`,
`createTrackedEffect` and a handler are not (ADR 0625). A call no role places is
uncertifiable. The callback itself runs from a microtask with no owner.

Generators are placed as for [no-write-in-pure-computation](no-write-in-pure-computation.md) (ADRs 0851, 0853).

A call that no row, discard or named silence ends is uncertifiable: `on-flush-unaccounted` (ADR 0936).

## Endings by code (ADR 0968)

An `onflush` call is accounted for, for this rule's Chrome code, only by an ending that answers that code: a row of its own table, an SC9012, the compiler's discard, a finding of a rule the dialect says claims the code, or a named silence that names it. A row of another table or a finding of another rule at the same place answers another code and ends nothing here, so this rule's own uncertifiable row (`on-flush-unaccounted`) stands beside it: never a violation, and it yields to any row of the rule that stands. A site whose rule claims no code here, or whose code has no policy in the dialect, is not ended by what stands at it.
