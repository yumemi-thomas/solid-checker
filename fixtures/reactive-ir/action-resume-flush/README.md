# action-resume-flush

**Claim (ADR 1051, SC2008).** An action whose generator's first statement is a `yield` of a value that is no thenable steps again at once: `@solidjs/signals@2.0.0-rc.21`'s `run` calls `inTransaction(t, q, () => step(r.value))` (`dist/dev.js:2365`), and `inTransaction` flushes when no action step is on the stack (`:2198-2208`). An action called on a `createEffect` apply's stack with no owner (`ownerless_effect_phase_origin`) runs that flush inside the flush draining effects: it drains nothing, and the dev build warns FLUSH_IN_EFFECT_CALLBACK at the action's call.

Proved: `YieldNumber` (`yield 1`), `BareYield` (`yield;`), `YieldString` (`yield "step"`, the action called with an argument). No row: `YieldPromise` and `YieldBinding` (the next step runs from a continuation), `FromSettled` (the flush throws another error there), `AsyncGenerator`, and `CodeBeforeYield` (a known miss: code before the `yield` may throw). Chrome rc.21: oracle case `action-resume-flush`.

Stubs: rc.21 package versions (`node_modules`, as `dead-literal-rows`); `solid-js.d.ts` as `owner-invoked-literal`.
