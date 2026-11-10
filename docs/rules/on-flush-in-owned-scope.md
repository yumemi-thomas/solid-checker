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
