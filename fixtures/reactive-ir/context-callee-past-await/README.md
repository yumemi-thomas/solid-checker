# context-callee-past-await

**Claim (ADR 0888).** A call written in the rest of a block that an `await` of its own async function dominates runs on that function's continuation, where no owner is current (ADR 0352): the callee gains no owned run from it. Before, the call edge kept the caller's owned bit as a possible run, and ADR 0577's owned-run throw read it as proven, so a provider-free read in the callee was SC4002 (`ContextNotFoundError`) where rc.21 throws `NoOwnerError`.

Violations: `PastAwaitCallee` (async helper awaited past the memo's await; its read before its own await) and `SyncCalleePastAwait` (a sync helper called twice past the await), SC4001 at the read, no SC4002. `BeforeAwaitCallee` (called before the memo's await): SC4002, the owned run throws `ContextNotFoundError`. `SkippableAwaitCallee` (the await is behind `if`): SC4002, and SC4001 stays an obligation (the owned run is reachable).

Runtime (rc.21 Chrome, R3e probes): the past-await forms throw `NoOwnerError: Context can only be accessed under a reactive root` (a reading effect logs it with `console.error`, a JSX read reaches the window); the before-await form throws `ContextNotFoundError`. Fuzzer row: `mx-census` `mx-context-not-found-aw-helper-late-x-direct-none`. Premises: the closed program is the `private` manifest.
