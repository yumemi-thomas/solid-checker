# owner-invoked-literal

**Claim (ADR 0347).** An immediately invoked function literal runs in its call, under the owner of the code around it: module scope and an unowned callback are unowned, a component body is owned. A plain IIFE creating an effect is a `missing-owner` violation where the effect is unowned: `moduleScope` and `DetachedIife` (inside `runWithOwner(null, …)`). `Owned` (component body) stays silent.

Silent: `generated` (a generator runs none of its body); `unused` (dormant); `AsyncOwned` (code before an async IIFE's first `await` runs under the component's owner).

Pinned violation: `asyncLate` (code after the first `await` has no owner, ADR 0333; rc.13 warns `NO_OWNER_EFFECT`).

Stubs: rc.13 `solid-js.d.ts` (as `unplaced-site-obligations`) and `runWithOwner` from `@solidjs/signals`; `Owner` is abbreviated.
