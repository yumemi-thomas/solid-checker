# fp-owner-nested-async-read

**Claim.** `reactive-read-after-await` blames a tracked computation only when
the async function *is* the computation. An async function nested in a tracked
callback (an async IIFE) or returned from it (`createMemo(() => async () =>
...)`) is not suspended by the primitive: the callback finishes before the
await, and the returned closure runs outside the tracking pass.

| Case | Finding | Why |
| --- | --- | --- |
| async IIFE inside `createTrackedEffect` | none | the callback is synchronous |
| `createMemo(() => async () => ...)` | none | the memo's value is the closure |
| `createMemo(async () => { await ...; return id(); })` | `reactive-read-after-await` | the computation itself awaits |

Stub as in `fp-owner-show-children-callback`; type-checks against the real rc.9
typings.
