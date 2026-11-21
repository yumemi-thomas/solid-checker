# effect-cleanup-rerun-reached

**Claim (ADR 1183).** A read in the body of the cleanup an inline apply returns (slot 1 of `createEffect` or
`createRenderEffect`) runs on the effect's rerun, in the apply's strict-read window (ADR 0539), with the owner alive, when: the
effect call runs on every completion of the component's body; its inline compute calls on every completion the accessor of a
`const [t, setT] = createSignal(<primitive literal>)` of that component, with no options; and `setT(<a different primitive
literal>)` runs on every completion of an inline `on…` handler of an intrinsic element, with no spread, `disabled` or `inert`, that
the component's returned element reaches through intrinsic elements' direct children (ADR 1132: a handler a user can reach is
reachable). SC1001 is a violation (reads only; owner codes keep their rows).

Violations: `EffectRerun`, `RenderEffectRerun`. Uncertifiable: `TwinSameValue` (the write sets the value it holds: no rerun),
`TwinUnderShow` (a flow component between), `TwinConditionalWrite`, `TwinDisabled`. Chrome:
`runtime-oracle/effect-cleanup-rerun-reached`. Stub: `effect-cleanup-disposal-only`'s.
