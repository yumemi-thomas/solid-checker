# leading-token-bind-patched

**Claim (ADR 0376 review, ADR 0338).** A standard `bind` result runs its target only while the project cannot replace `Function.prototype.bind`. Here it does, so `setupBound.bind(null)()` and `trackBound.bind(null)(…)` prove no owner edge or tracker invocation: no SC4001 (silent, the escape class in `docs/open-work.md`). Control: `setupCalled.call(null)` is SC4001, since the tripwire covers `bind` only. Owners: `invoked_function_span`, `bound_function_span` in `owners/missing_owners.rs`. Stub: `leading-token-callback-identity`'s.
