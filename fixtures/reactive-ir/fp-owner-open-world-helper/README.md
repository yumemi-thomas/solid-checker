# fp-owner-open-world-helper

**Claim.** An exported non-component function has callers the analysis cannot see, so an `onCleanup`
in it is uncertifiable, not a proven missing owner. A private helper reached only from exported
functions inherits that open world along the call edge.

- `missing-owner` uncertifiable: `onCleanup` in an exported function; in a private helper called only
  from exported functions.
- `missing-owner` violation: `onCleanup` in a helper called from a `createEffect` apply callback (the
  apply runs with no owner).

Stub as in `fp-owner-show-children-callback`; type-checks against the real rc.9 typings.
