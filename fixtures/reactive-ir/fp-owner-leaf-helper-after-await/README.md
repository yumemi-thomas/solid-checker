# fp-owner-leaf-helper-after-await

**Claim.** A helper called synchronously from `onSettled` is credited with the
`onCleanup` calls in its *synchronous extent* only. Past the first `await` the
call runs from a promise continuation: the leaf scope is gone and there is no
owner, so the real outcome is `NO_OWNER_CLEANUP` (`missing-owner`'s claim), not
`CLEANUP_IN_FORBIDDEN_SCOPE`. Both rules used to fire on the one operation with
contradictory claims.

| Case | Finding | Why |
| --- | --- | --- |
| `onCleanup` after `await` in a helper called from `onSettled` | no `leaf-owner-forbidden-call` | continuation, not the leaf scope |
| `onCleanup` before the first `await` | `leaf-owner-forbidden-call` | still synchronous inside the leaf |

Stub as in `fp-owner-show-children-callback`; type-checks against the real rc.9
typings.
