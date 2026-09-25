# A local clearing helper keeps its clear across the forwarding seam

`runUntracked(fn) { return untrack(fn) }` is the one-line wrapper most of the
ecosystem writes. Its own row is `inline` + `untracked`. Forwarded under an
enclosing wrapper, the seam used to compose the enclosing chain alone and
substitute that answer for the helper's, so `createMemo(() => runUntracked(cb))`
published `tracked` -- an affirmative wrong claim, since 2.0's `createMemo`
computes during the creating call and `cb` runs inside `untrack`
(docs/precision-backlog.md, "A package-local transparent wrapper around the real
`untrack`"). The helper's row is now the innermost wrapper of the composition.

The same fixture pins the `deferred` half of ways-to-improve step 7: a deferral
says `untracked` only where it is proven to run with no caller's listener.

| export | row | why |
| --- | --- | --- |
| `untrackedThroughLocalWrapper` | same-stack, `untracked` | the helper clears, and the memo computes during the call |
| `trackedThroughLocalHelper` | same-stack, `tracked` | negative control: `runNow` calls `fn` bare, so the memo subscribes it (the same shape `callback-deferred-untracked-chain` pins under `createEffect`) |
| `listen` | queued, `ambient-at-execution` | `addEventListener`: a synchronous `dispatchEvent` runs the listener on the dispatcher's stack |
| `bindArgument` | queued, `ambient-at-execution` | `Function.prototype.bind`: the bound function is called by whoever holds it, and hands `callback` on from there |
| `later` | queued, `untracked` | control: `setTimeout` runs its callback from a task, on an empty stack |
| `throughUnclassified` | `callbacks` open | an unclassifiable wrapper -- the caller's own `schedule` -- keeps the row open even though the helper inside it clears |

The `solid-js` stub is `untrack` and `createMemo` byte-faithful to
`solid-js@2.0.0-rc.3` (the stub's header names the source lines), and
`index.ts` is `tsc --strict --noEmit`-clean against both the stub and the
published typings under `lib: ["ES2022", "DOM"]`.
