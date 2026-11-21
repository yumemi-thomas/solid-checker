# named-callback-reporting-run

**Claim (ADR 1166).** A read written in the body of a callback (not a nested function, an instance field or a parameter default; the
callback neither `async` nor a generator) that is the whole initializer of a `const` binding runs untracked where a run of that
binding provably executes it: a direct call `f()`, or a literal `[f].forEach((g) => g())` / `[f].map((g) => g())` (ADR 1124's carrier,
`map` now included), written after the declarator in the function that declares the binding, on every completion of that function
and of the carrier's arrow, at a position where a read written in its place reports. A read rule asks whether some run is untracked,
so the callback's other entries (a memo's compute, `onSettled`, an effect's compute) do not matter: SC1001 is a violation.

Violations: `StoreMemoCalledInBody` (it had a violation from the call summary and an uncertifiable row beside it),
`StoreMemoCarriedInBody`, `StoreMemoMappedInBody`, `StoreSettledCarriedInBody`, `StoreEffectComputeCarriedInBody`,
`StoreNamedExpressionCarriedInApply`, `SignalMemoCarriedInBody`. Clean: `StoreMemoMappedInHandler` (the `map` run is in a handler).
Uncertifiable twins: `TwinDeadCall` (`if (false) f()`), `TwinConditionalCarrier`, `TwinLoopCall`, `TwinFilter` (no carrier),
`TwinLetBinding` (the binding is reassigned), `TwinNestedRead` (the read is in a nested function the run does not execute), `TwinApplyCreationFirst` (in an effect's apply, outside
the body's rendering, a call before the run, here `createMemo(f)`, throws PRIMITIVE_IN_EFFECT_CALLBACK first: no call may stand
between the declarator and such a run).
Chrome: `runtime-oracle/named-callback-reporting-run`. Stub: `foreach-carrier`'s, plus `createStore`.
