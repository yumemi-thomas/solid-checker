# resolve and until from a tracked effect

ADR 0641. On rc.21 `resolve()` and `until()` under `createTrackedEffect` throw "Cannot call resolve
inside a reactive scope", the observer guard, so the claim is SC2004 and SC2005, also through an
exact helper; `leaf-owner-forbidden-call` has no such mechanism there. A primitive that does need an
owner stays that rule's, and an event handler has no observer. `onSettled` runs under `untrack`, so no observer guards it: there the call stays `leaf-owner-forbidden-call`.
