# leaf-scope-helper-callback

**Claim (ADR 0492).** A primitive or `onCleanup` written in a callback that a project helper calls in its own body (ADR 0469's in-place climb, through any number of such helpers) runs in a leaf owner's callback when the outermost helper call sits directly there, so `leaf-owner-forbidden-call` reports it where it is written. Violations: `TrackedHelper`, `SettledTwoHelpers`, `CleanupInHelper`. Silent: `Deferred` (the helper hands its callback to `setTimeout`), `Stored` (the helper keeps it), `AfterAwait` (the helper calls it after an `await`), `InHandler` (the helper call runs in a listener). An IIFE, an inline standard-library callback and a named function keep their own paths (`leaf-scope-helper-completion`, `leaf-scope-host-callbacks`).

Runtime: rc.17 throws `PRIMITIVE_IN_FORBIDDEN_SCOPE` (and `CLEANUP_IN_FORBIDDEN_SCOPE`) for the helper forms (the misuse matrix, scripts/fuzz, Chrome-confirmed). Stub: `leaf-scope-helper-completion`'s.
