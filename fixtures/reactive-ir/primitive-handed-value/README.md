# primitive-handed-value

**Claim (ADR 0825 amended, batch 11).** A Solid primitive handed to a call as an argument (`untrack(flush)`, `run(flush)`, `[1].forEach(flush)`, `run(...[flush])`, a value a map the call constructs holds) is run by the call's callee, and the frame Chrome (rc.21) reports for that run stands at the call (V8 puts it at the call's start). SC9012 `primitive-value` stands at the argument, and the judges cover a withholding at a frame's position only, so the pattern lane judged these programs false clean (`scripts/fuzz/patterns/byvalue/*`). For a primitive the binder confirmed, a companion SC9012 row, `primitive-handed`, stands at the start of the call the primitive is handed to: uncertifiable, never a violation. The census does not count it as a row of the call (the call may hand a source too).

`App.tsx`: `Untracked`, `Helper`, `ForEachCallback`, `Spread` and `Stored` hand `flush` to a call in an effect's apply (FLUSH_IN_EFFECT_CALLBACK); `Resolving` hands `resolve` to a project function in a memo (RESOLVE_IN_REACTIVE_SCOPE). `Deferred` hands `flush` to a microtask, a `then` and a timer: Chrome keeps quiet, each call has its rows, none is a violation.

A primitive whose identity the binder did not confirm (`primitive-identity-unproven`) files no companion: those values are no primitive.
