# write-in-deferred-callback

**Claim.** `SC2001` reports a write in a function literal written in a component body only when the literal provably runs during the body (`nested_literal_runs_during_body`): handed to a project function that invokes it during the call, a standard-library inline callback, an IIFE, a primitive's or control-flow callback, or a named closure called in the body; elsewhere the write is unclassified and uncertifiable (timer-stored callbacks, returned closures), never a violation; an `onClick`-only helper reports nothing. Stub: `createSignal` verbatim from rc.9.
