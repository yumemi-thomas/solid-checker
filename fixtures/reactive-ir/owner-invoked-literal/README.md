# owner-invoked-literal

**Claim (ADR 0347).** An immediately invoked function literal runs in its call, so it runs under the owner of the code around the call: module scope is unowned, an unowned callback is unowned, a component body is owned. A plain IIFE that creates an effect is therefore a `missing-owner` violation exactly where the effect is unowned: `moduleScope` (module scope) and `detachedIife` (inside `runWithOwner(null, …)`). `Owned` (in a component body) stays silent.

Withheld, so silent: a generator IIFE (`generated`), because calling it runs none of its body; an unexported function nothing calls (`unused`), which stays dormant; an async IIFE's code before its first `await` runs in the call under the component's owner (`AsyncOwned`).

Pinned as a violation: `asyncLate`. The code after an async IIFE's first `await` is a continuation with no owner (ADR 0333), and Solid's rc.13 dev runtime warns `NO_OWNER_EFFECT` there (runtime oracle, `async1` probe). An IIFE's async body is not exempt from that rule, so this case is a positive, not a negative.

Stub declarations are copied from published rc.13 (`solid-js.d.ts`, the same declarations as `unplaced-site-obligations`), plus `runWithOwner` from `@solidjs/signals/dist/types/core/core.d.ts`. `Owner` is abbreviated: the cases pass only `null`, so no member is read.
