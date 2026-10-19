# fp-owner-nested-async-read

**Claim.** `reactive-read-after-await` blames a tracked computation only when the async function *is* the
computation. An async function nested in a tracked callback (an async IIFE) or returned from it
(`createMemo(() => async () => ...)`) is not suspended by the primitive.

- none: an async IIFE inside `createTrackedEffect`; `createMemo(() => async () => ...)`.
- `reactive-read-after-await`: `createMemo(async () => { await ...; return id(); })`.

Stub as in `fp-owner-show-children-callback`; type-checks against the real rc.9 typings.
