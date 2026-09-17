# Schedule and tracking are independent axes

`inline`/`same-stack` promises the export invokes the callback **before it
returns**. `tracked` says reads inside it **subscribe**. They are different
questions, and this fixture is the grid that keeps them apart — every export is
`createEffect`, `createTrackedEffect`, `createMemo`, `createSignal`, `onCleanup`
or `untrack` in some arrangement, so nothing varies except which wrapper the
callback sits under.

| | tracked | untracked |
| --- | --- | --- |
| **same-stack** | `trackedShape`, `memoInsideUntrack`, `trackedThroughLocalHelper` | `mountShape`, `inlineShape`, `memoShape`, `derivedSignalShape`, `untrackedThroughLocalUntrack` |
| **queued** | `deferredTrackedShape` | `cleanupShape` |

All four cells are occupied on purpose. A rule that collapsed the axes — "a
tracked callback runs later", or "a clearing wrapper makes a callback earlier" —
is right in one cell and wrong in the others.

The pair to read first is `trackedShape` and `mountShape`. They differ by one
wrapper and one word: adding `untrack` inside the effect moves the *tracking*
axis and leaves the schedule alone. The pair to read second is `trackedShape`
and `deferredTrackedShape`, which differ on the schedule axis alone.

## Why this replaces a 1.x fixture rather than porting one

The Solid 1.x fixture of this name was deleted with that dialect (ADR 0110) and
could not be transcribed: three of its exports lose their premise in 2.0.

- **`mountShape` answers differently, and both answers are right.** It was 1.x's
  `onMount`, `createEffect(() => untrack(fn))`, and 1.x's `createEffect` defers
  its compute (`AfterCall`), so the callback was queued. 2.0's runs its compute
  **during the creating call**: `Solid2::tracked_callback_timing` cites the
  bytes — `createEffect` reaches `effect()`, which calls `recompute(node, true)`
  before queueing the *effect* function. So the same source text earns
  `same-stack` here. This is the headline dialect difference on this axis.
- **`unestablishedScheduleShape` has no counterpart.** 1.x's `createSignal(fn)`
  *stored* the function and never invoked it, so no schedule was proven and the
  chain refused. 2.0 selects the derived overload on
  `typeof first === "function"` and models it as a compute slot, so the chain
  answers. `derivedSignalShape` pins the 2.0 reading instead; pinning a genuine
  refusal needs a primitive the dialect states **no** timing for, and
  `createStore`/`createOptimisticStore` are the documented candidates.
- **`mergePropsShape` is gone.** 2.0's `merge` is not modelled as a
  callback-taking primitive at all.

`memoShape` and `renderEffectShape` claimed `inline` from measurements against
1.x runtime bytes. `memoShape` is kept because 2.0 states `createMemo` is
`DuringCall` in the dialect, which is the same claim from an audited source;
`renderEffectShape` is not, because nothing here would distinguish it from
`trackedShape`.

## What it pins that nothing else did

The direct-invocation rung dropped the schedule column for a `tracked` word and
took the consumer's historical `queued` default. Every tracked callback
therefore published "runs after the export returns", including the four 2.0
primitives the dialect audits as running during their own call. With 1.x retired
that was every tracked primitive except `createTrackedEffect`.

`trackedShape` and `deferredTrackedShape` are what catch it: before the fix both
answered `queued` and the grid's bottom-left and top-left cells collapsed into
one. `docs/precision-backlog.md` recorded the gap and named this fixture's
predecessor as the thing that would distinguish the two rungs — which it could
not do while it was deleted.

## Stub faithfulness

`node_modules/solid-js/index.d.ts` transcribes the six declarations this fixture
calls, each cited by file and line in the stub's header, from
`@solidjs/signals@2.0.0-rc.3`. Nothing on the argument side is reduced — above
all `createSignal`'s derived overload, which is what makes `derivedSignalShape`
a compute slot rather than a stored value. `index.ts` and `solid-js/runtime.ts`
type-check clean under `--strict` against the real `solid-js@2.0.0-rc.3` install
as well as against this stub.

## `solid-js/runtime.ts`

Its *path* is load bearing twice: `solid_primitive_declaration` resolves a
declaration whose path carries an exact `solid-js` component and whose name the
dialect declares, and being a summary node in the same file as its callers
routes them through the local-callee forwarding seam rather than the
primitive-argument branch. Both are dialect-neutral and outlived the retirement
unchanged. `untrackedThroughLocalUntrack` and `trackedThroughLocalHelper` are
the same shape through that seam with a clearing and a non-clearing wrapper; if
the seam ever lost wrapper identity they would collapse onto one answer.
