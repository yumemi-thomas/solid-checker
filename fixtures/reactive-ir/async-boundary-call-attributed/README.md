# async-boundary-call-attributed

**Claim (ADR 0462).** An async memo read written directly in a named local function is rendered where a call of that function sits in rendered JSX (through up to four such functions). Nothing proves the read runs pending while the call renders, so `async-outside-loading-boundary` is uncertifiable at the call, never a violation. Uncertifiable: `Wrapper`, `Closure` (an IIFE), `TwoLayers`, `UnderShowAndFor` (both calls). Violation: `Direct` (the read itself, unchanged). Silent: `UnderLoading`, `InHandler` (an event handler), `Probed` (`isPending`), `Declared` (`loadingValue`), `Sync`, `Reassigned` (a written binding).

Runtime: rc.17 reports `ASYNC_OUTSIDE_LOADING_BOUNDARY` for the wrapper and closure forms (the misuse matrix, scripts/fuzz). Stub: `fp-owner-loading-cover`'s, plus rc.17's `isPending`.
