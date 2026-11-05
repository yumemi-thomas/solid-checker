# Each effect constructor's function slot

ADR 0701. SC7001 asks the dialect where each primitive takes its effect function (`effect_function_slot`). `createRenderEffect`
takes it second and reads no bundle: `undefined`, a number and an `{ effect }` literal behind `as any` are violations (Chrome:
`node._effectFn is not a function`). `createReaction` takes it first and calls it only inside the tracker it returns, so a
non-callable one is uncertifiable, blocker `effect-returned-function-run-unproven`. Callable arguments, a reaction's bundle and a
shadowed local stay clean.
