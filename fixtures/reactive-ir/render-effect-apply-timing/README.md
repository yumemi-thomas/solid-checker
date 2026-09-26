# render-effect-apply-timing

**Claim.** `createRenderEffect(compute, apply)` runs its first `apply` during
the call, under the caller's owner, and every later `apply` from the flush with
no owner. `createEffect(compute, apply)` queues every `apply`, first included.

Measured on the published bytes, `@solidjs/signals@2.0.0-rc.3` (manifest-verified
against `benchmarks/package-contract-v2/phase0/rc3/`) and `2.0.0-rc.9`, dev and
prod client builds. `effect()` ends
`recompute(node, true); !options?.defer && (… EFFECT_USER || options?.schedule ?
node._queue.enqueue(…) : runEffect(node))` (rc.3 `dist/dev.js:5285-5289`,
`dist/prod/core/effect.js:16-17`; rc.9 `dist/dev.js:1612-1620`), and
`runEffect` sets neither the owner nor the listener. A Node probe records
`compute,apply,returned` with no owner, in a root, in a memo compute, in a
render-effect compute and in a component body under `render`; `getOwner()` in
the first apply is the caller's owner and `null` in a later one.

What that means for each rule here:

| Case | Finding | Why |
| --- | --- | --- |
| cleanup or effect in a render-effect apply, module scope | `SC4001` violation | no run has an owner |
| cleanup or effect in a render-effect apply, component body (named and namespace import) | `SC4001` uncertifiable | the first run is owned; only a later run, which needs a compute source to change, is detached |
| cleanup in a render-effect apply inside `createRoot` | `SC4001` uncertifiable | as in a component body; the root-contained shapes are `render-effect-apply-root`'s claim |
| cleanup in a `createEffect` apply, component body | `SC4001` violation | the apply is always queued, so always detached |
| cleanup in a render-effect compute | none | owned by the render effect |
| write in a render-effect apply, component body | none | the first run throws `REACTIVE_WRITE_IN_OWNED_SCOPE` (dev) only if it runs during the call, which needs the compute to settle synchronously on its first pass; that is not proven, so the write is left unclassified rather than reported or certified |
| write in a `createEffect` apply, or in a render-effect apply created in an event handler | none | legal on every run |
| write in the component body | `SC2001` violation | control |
| read in a render-effect apply, inside a helper | `SC1001` violation at the read | the runtime opens its `"an effect callback"` strict-read window on every apply run, the first included |
| that helper called in a component body | `SC1001` violation through the helper | the first apply runs during the helper's call with the body's (absent) listener, so the helper reads for its caller; the same shape `runWithOwner`'s callback already has |
| that helper called in a memo compute | none through the helper | the read subscribes the memo (probed: the memo re-runs when the signal changes) |
| read in a `createEffect` apply inside a helper called in a component body | `SC1001` at the read only | the apply runs from the flush, outside the caller's extent |

The render effect created in the event handler also reports its own `SC4001`
violation (an effect with no owner), which is true and not this fixture's claim.

**Stub.** `solid-js.d.ts` copies the effect constructors, their callback and
option types, `onCleanup`, `Disposable` and `createRoot` verbatim from the
published rc.3 declarations; the reductions are listed in the file and touch no
premise. `App.tsx` type-checks cleanly both against this stub and against the
real `solid-js`/`@solidjs/signals`/`@solidjs/web` rc.3 and rc.9 typings.
