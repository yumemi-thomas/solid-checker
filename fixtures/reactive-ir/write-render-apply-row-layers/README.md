# write-render-apply-row-layers

**Claim (ADR 0460).** A write in a function that a `createRenderEffect` apply or a `For` row runs in place (an IIFE, a helper that calls its callback, a named closure the callback calls, `untrack`) is placed where that code runs, never by the deferred role the callback's lexical scope lends it.

Violations (`no-write-in-pure-computation`; rc.17 throws `REACTIVE_WRITE_IN_OWNED_SCOPE` at each): in a row's JSX child, an IIFE that calls `refresh` (direct, `S.refresh`, a `const { refresh: refreshD } = S` slot), a signal setter through a helper, a store setter, a setter under `untrack`; a named closure the row body calls.

Uncertifiable: the same layers in a render effect's apply (closure, IIFE, helper, destructured `refresh`). rc.17 runs the apply's first run synchronously under the component's owner and throws, but only when the compute settles on its first pass outside a lane and transaction (`@solidjs/signals` `dist/dev.js:1851-1870`, `recompute`), which is not proven; later runs are legal.

Silent: a `createEffect` apply's closure (queued, no owner); in a row, an event handler, a timer callback and an effect apply; a write past an `await` in an async layer of a render effect's apply (a continuation with no owner).

Uncertifiable, not a violation (review of ADR 0460): a write past an `await` in an async layer of a row (an async IIFE, an async callback a helper runs, an inner IIFE after the await). At run time it is a continuation with no owner and legal; the checker withholds the row's placement there and does not prove the continuation, so it fails closed.

tsc: clean against the stub (copied from `write-cleanup-handle-call`, with `For`'s default-keyed overload transcribed from rc.17 `types/client/flow.d.ts:32`). Runtime: each component checked alone on the fuzzer's rc.17 fast path (Happy DOM); the violations and the uncertifiable cases report the code, the silent ones report nothing.
