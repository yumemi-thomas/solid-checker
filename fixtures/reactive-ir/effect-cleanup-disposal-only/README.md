# effect-cleanup-disposal-only

**Claim (ADR 1167).** A read written in the body of the cleanup an effect's (or a render effect's) inline apply returns, where the
compute provably tracks nothing (ADR 0539's `effect_never_reruns`), runs only when the effect's owner is disposed: no rerun calls the
cleanup and no other code can name it. A disposal runs it outside any strict-read window unless project code can dispose an owner on
a stack of its own (ADR 0695, `project_may_dispose`, the premise `onCleanup` rests on): SC1001 is clean. Only the read role takes the
answer; a `For` row's disposal runs the cleanup owned, which owner codes read differently.

Clean: `ShowRemoved`, `ForRowRemoved`, `RenderEffectCleanup` (a signal read). Uncertifiable: `TwinComputeTracks` (a rerun may call it
in the apply's window), `TwinNamedApply`, `TwinNestedRead`, `TwinAsyncCleanup`; and `effect-cleanup-disposal-reachable`, where a
`createRoot` that takes `dispose` makes every disposal reachable from project code (disposed in the body, in its window). Chrome:
`runtime-oracle/effect-cleanup-disposal-only`. Stub: `foreach-carrier`'s, plus `createStore`, `Show`, `For` and `createRoot`'s `dispose`.
