# async-unsuspendable-call-attributed

**Claim (ADR 0490).** An async memo read written directly in a named local function is read where a call of that function runs (through up to four such functions). A call in the component body's strict-read window or in a leaf owner (`createTrackedEffect`, `onSettled`) reads the pending source unsuspendably, as a direct read there does, so `pending-async-unsuspendable-read` is a violation at the call. Violation: `BodyClosure`, `BodyTwoLayers` (at `f2()`), `TrackedWrapper`, `SettledWrapper`. Silent: `InHandler` (an event handler), `Probed` (`isPending`), `NeverCalled` (handed to JSX, never called here), `Reassigned` (a written binding).

Runtime: rc.17 throws `PENDING_ASYNC_UNTRACKED_READ` for the body forms and `PENDING_ASYNC_FORBIDDEN_SCOPE` for the leaf-owner forms (the misuse matrix, scripts/fuzz, Chrome-confirmed). Stub: `async-boundary-call-attributed`'s.
