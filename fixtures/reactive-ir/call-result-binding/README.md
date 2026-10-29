# call-result-binding

**Claim (ADR 0506).** `const wrapped = wrap(() => set(1))` names the arrow for identity only (a server function a `query` wraps, a `lazy` loader's tag): the binding holds the call's result, so `wrapped()` is no call of the arrow and places no write there (`CallSitesOf` keeps such a binding's render sites, not its calls). Before, the call graph took `wrapped()` as the arrow's call, and a write in an arrow the wrapper never runs was a violation where `wrapped()` was called. Uncertifiable: `NeverRuns` (the wrapper returns another function), `UnknownWrapper` (a declared-only wrapper). Violations: `IdentityWrapper` (ADR 0346's exact passthrough), `DirectBinding` (the arrow is the binding's value).

Stubs: `node_modules` and `jsx.d.ts` copied from `effect-apply-first-run` (rc.17 published declarations).
