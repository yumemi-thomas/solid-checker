# reaction-settled-run

**Claim (ADR 1203).** rc.21's `onSettled` returns without throwing: it runs its callback on no path before it returns (a lazy
`trackedEffect` whose first run is enqueued, or a queued callback; `Dialect::call_returns_without_throwing`). So an exact call of
it, with plain identifier or function-literal arguments, between a closure's declaration and its run does not interrupt the run
(ADR 1166's `reporting_run_role`): in a `createReaction` effect the read runs in the effect's untracked pass, where rc.21 warns
STRICT_READ_UNTRACKED.

Violations (uncertifiable `function-run-unplaced` before): `SettledCarried`, `SettledCalled`, `SettledLiteral`. Chrome:
`runtime-oracle/reaction-settled-run`.

Uncertifiable (the run stays unproven):
- `MemoBefore`: `createMemo(f)` throws PRIMITIVE_IN_EFFECT_CALLBACK in the effect before the run (rc.21; the fuzz programs
  `mx-strict-read-untracked-reaction-store-byname-*`, Chrome).
- `SettledMember`: the argument is a property read, which may throw (here a getter does) before `onSettled` is called.
- `ShadowedSettled`: a local `onSettled` that throws shadows the import, so the call is no primitive's.

Stub: `reaction-closure-read`'s, with `createMemo`'s plain overload.
