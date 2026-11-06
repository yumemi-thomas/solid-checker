# Each effect constructor's function slot

ADR 0701 (`effect_function_slot`). `createRenderEffect` takes it second, no bundle: `undefined`, a number and an `{ effect }`
literal, each behind `as any` (so not `tsc`'s), are violations (Chrome: `node._effectFn is not a function`, f01, f02). `createReaction` calls its first argument only in its tracker: uncertifiable,
`effect-returned-function-run-unproven`. Callables, a reaction's bundle and a shadowed local stay clean.
