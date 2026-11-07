# stored-throw-observation

**Claim (ADR 0893).** rc.21 `resolve`, `until`, a static `dynamic` source and `useContext` fail with a plain `throw`, no diagnostic at the call. A throw in a memo or another storing compute stays on the node until a read rethrows it, so a violation needs a run whose throw reaches a report: an effect's or render effect's compute, a read in one, a read in a handler or a component body. One entry that reports keeps the violation.

Violations: `ReadInEffect`, `ReadInJsx`, `ReadInHandler` (Chrome confirms it on a dispatched click), `ChainRead`, `ReadThroughHelper`, `ReadUnderUntrack`, `EffectEntry`, `ContextRead`, `LoggingBoundary` (`Errored` logs a zero-arity fallback).

Obligations (uncertifiable, `computation-error-unobserved`): `Unread`, `ContextUnread`, `ChainUnread`, `DeferredUnread`, `ReadInTry`, `TryAboveHelper`, `ReadInProbe` (`isPending` takes every error but a not-ready), `ErrorArm`, `HandlingBoundary`, `HandlingBoundaryAboveMount`, `ReadInGetter`, `ReadPastAwait` (fires as an unhandled rejection; not proven here), `BodyEntry`, `AliasedComputeUnread`, `TryAroundSite`, `TryAboveThrower`, `ReferenceOnly`, `PropRunInChildTry`, `ThenCatch`, `RenderKid`, `TrackedKid` (a render or tracked effect escalates to the boundary above).

Silent: `ShadowedMemo`, a local function with the primitive's name.

Runtime: rc.21 Chrome, probes `cases/mu`, `cases/mv` and fuzzer contexts `st-*` (`scripts/fuzz/flip-read.mjs`). Premises: the closed program is the `private` manifest.
