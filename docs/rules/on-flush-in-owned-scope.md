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

A generator is placed at its steps, never at iterator creation (ADR 0851). Exported iterators, mixed step contexts, and a named generator also handed on as a value retain `function-run-unplaced` obligations; the creation call cannot restore a rejecting compute role (ADR 0853). Some synchronous consumption/first-segment write positions also remain obligations where the write-role proof does not preserve the step context.
