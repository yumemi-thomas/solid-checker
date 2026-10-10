# fp-owner-open-world-helper

**Claim.** An exported non-component function has callers the analysis cannot
see, so an `onCleanup` in it is uncertifiable, not a proven missing owner. A
private helper reached only from exported functions inherits that open world
along the call edge; before, the unowned context carried no provenance and the
helper's `onCleanup` was reported as a proven violation while the identical
call written in the exported function was not.

| Case | Finding | Why |
| --- | --- | --- |
| `onCleanup` in an exported function | `missing-owner` uncertifiable | unseen callers |
| `onCleanup` in a private helper called only from exported functions | `missing-owner` uncertifiable | same open world, one edge away |
| `onCleanup` in a helper called from a `createEffect` apply callback | `missing-owner` violation | the apply callback runs with no owner: proven |

Stub as in `fp-owner-show-children-callback`; type-checks against the real rc.9
typings.
