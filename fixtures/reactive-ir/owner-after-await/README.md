# owner-after-await

**Claim.** An owner-requiring operation (`onCleanup`, an effect, a contract
call with an owner requirement) that runs after an `await` on every path
through its async function's body has no owner, whatever owner the function was
entered with. It is a proven `SC4001` violation.

Code after an `await` resumes from a promise continuation on an empty stack.
The positions are the producer's `calls_after_await` dominance fact, the one
`reactive-read-after-await` already proves reads with; the owner half is the
dialect's fresh-stack answer (`Dialect::fresh_stack_callback_owner`), the one
`fresh-stack-scheduler-owner` pins for host schedulers. See
`owners::AwaitContinuations`.

| Case | Finding | Why |
| --- | --- | --- |
| `onCleanup` after `await` in a helper a component calls | `missing-owner` violation | continuation, no owner |
| `createEffect` after `await` | `missing-owner` violation | continuation, no owner |
| `onCleanup` after `await` inside `try` | `missing-owner` violation | reached only after the await |
| exported helper, no caller in the project | `missing-owner` violation | no caller can supply an owner to a continuation |
| `onCleanup` before the first `await` | none | still synchronous under the caller's owner |
| `await` on one `if` branch only | none | the other path runs synchronously |
| `await` in `try`, cleanup after a `catch` that does not await | none | the catch path does not await |
| owner captured before the `await`, restored with `runWithOwner` | not a violation | the cleanup is in a nested closure, not an after-await call |

Measured on the published `@solidjs/signals` 2.0.0-rc.0, rc.3, rc.6, rc.8 and
rc.9 dev builds and the rc.9 prod build, under Node 24: inside a `createRoot`,
`getOwner()` is non-null before the `await` and `null` after it; an
`onCleanup` after it raises `NO_OWNER_CLEANUP` (dev) and does not run when the
root is disposed, while one before it does; a `createEffect` after it raises
`NO_OWNER_EFFECT` (dev).

`onSettled`'s returned cleanup keeps the enclosing context: whether an unowned
settle registers is a separate question this fixture does not pin.

The stub is `fp-owner-leaf-helper-after-await`'s plus `getOwner` and
`runWithOwner`, verbatim from rc.9; `App.tsx` type-checks against the real
`solid-js@2.0.0-rc.9` typings with no diagnostic.
