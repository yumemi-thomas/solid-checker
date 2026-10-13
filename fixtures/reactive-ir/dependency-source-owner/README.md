# Dependency source: owner requirements at the project's call (ADR 0290)

Claim: an owner requirement inside a package analyzed as source
(`.solid-checker/runtime.json` selects `dep-timer`) is reported at the
project's call that runs it there, under that call's owner context. The
package's own copy is not reported (ADR 0284). The attribution applies
ADR 0275 at the call boundary and ADR 0280 on the first hop: the operation,
and every call on the way to it, must be on its function's every-completion
path, or the project call's arguments must decide the branch it sits in.

| Case | Call | Expected |
| --- | --- | --- |
| `NumericInHandler` | `useTicker(fn, 1000)` in `onClick` | violation, `onCleanup` (through `register.js`); the effect branch is unreachable |
| `AccessorInHandler` | `useTicker(fn, delay)` with a signal | uncertifiable, both operations: a signal decides no `typeof` |
| `InBody` | in a component body | clean |
| `MaybeInHandler` | `useMaybe(flag, fn)` | uncertifiable, `onCleanup` on some calls |
| `LaterInHandler` | `useLater(fn)` | not attributed: the package schedules the unowned callback itself |
| `ArrowInHandler` | `useTicker(fn, () => 1000)` | violation, the effect; the cleanup branch is unreachable |

Without the flag, the package has no contract, and only `AccessorInHandler`
reports an uncertifiable (`reactive-source-uncaptured`).

`solid-js.d.ts` is `context-member-identity`'s stub plus `onCleanup` and
`Disposable` from `summary-completion-read` (claim-bearing signatures copied
from published 2.0.0-rc.13). `App.tsx` passes `tsc --noEmit`.
