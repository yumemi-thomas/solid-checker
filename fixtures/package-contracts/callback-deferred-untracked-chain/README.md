# A clearing wrapper inside a tracked one: order decides, and so does the tracked wrapper's schedule

The companion to `callback-untracked-wrapper`. That fixture pins that a
synchronous clearing wrapper alone is `inline`; this one pins what happens when
such a wrapper is nested, which is where the answer stops being derivable from
any single wrapper.

The shape that matters is solid-js's own `onMount`
(`dist/solid.js:485-487`):

```js
function onMount(fn) {
  createEffect(() => untrack(fn));
}
```

The generated contract claimed `tracked`. Two facts were being conflated:

- `untrack` clears the listener, so nothing `fn` reads subscribes the effect —
  the callback is **not** tracked;
- `createEffect` schedules its compute, so `fn` has **not run** when `onMount`
  returns — the callback is deferred.

The generator took the enclosing `createEffect` callback's lexical *tracking
scope* and published it as the callback's *schedule*. The repo's own reviewed
map states `deferred` for `onMount`, so the generator disagreed with the audited
oracle in the same tree; `docs/precision-backlog.md` lists it as one of the
eight named divergences.

The rule is a fold over the chain of enclosing callback positions, innermost
outward, and its order-sensitivity is the point:

| Export | claim | chain, innermost first |
| --- | --- | --- |
| `mountShape` | `deferred` | clearing, then **deferring** tracked — `onMount` verbatim |
| `mountShapeArrow` | `deferred` | same, with an explicit arrow so the claim does not depend on identity forwarding |
| `cleanupShape` | `deferred` | clearing inside genuinely-later; deferral is sticky |
| `memoShape` | `inline` | clearing, then **eager** tracked — same chain shape as `mountShape`, opposite answer |
| `renderEffectShape` | `inline` | the second eager 1.x primitive, with `createRoot` as the clearing half |
| `mergePropsShape` | `inline` | the component-defaults idiom, eager because `mergeProps` memoizes its function sources |
| `unestablishedScheduleShape` | `{"status":"unknown"}` | clearing, then a tracked wrapper the dialect states **no** schedule for |
| `unestablishedDirectShape` | `{"status":"unknown"}` | the same refusal reached through the direct-invocation rung |
| `memoInsideUntrack` | `tracked` | **tracked, then clearing** — the memo subscribes what runs inside it and an outer `untrack` cannot undo that |
| `trackedShape` | `tracked` | negative: drop the clearing wrapper |
| `inlineShape` | `inline` | negative: drop the deferring wrapper |

`memoInsideUntrack` is the case that rules out the simpler rule ("any clearing
wrapper anywhere means not tracked"), which would answer `deferred` there and be
wrong.

## Tracked does not mean later

`mountShape` and `memoShape` have the *same* chain shape — a clearing wrapper
inside a tracked one — and opposite answers, because a tracked wrapper's own
schedule is a separate fact from its attribution and the fold needs both. In
1.x only `createEffect` defers: `createMemo` (`dist/solid.js:244-256`),
`createRenderEffect` (`:218-221`) and `createComputed` (`:214-217`) all call
`updateComputation(c)` on the creating call, and `mergeProps` (`:1329`) wraps
each function source in a `createMemo`, so it is eager for the same reason.
`createEffect` (`:222-229`) is `Effects ? Effects.push(c) : updateComputation(c)`,
and every owner-backed context takes the push branch — `createRoot` runs its
body through `runUpdates(updateFn, true)` (`:192`), which installs `Effects = []`
(`:820`).

Measured against solid-js@1.9.14 from `rust/target/tsc-oracle/v1` under
`--conditions browser`, with the probe worker's own observation shape:
`createMemo(() => untrack(cb))`, `createMemo(() => createRoot(() => cb()))`,
`createRenderEffect(() => untrack(cb))`,
`createRenderEffect(() => createRoot(() => cb()))` and
`mergeProps({a:1}, () => untrack(cb))` all report `ranDuringCall` with no re-run,
i.e. **`inline`**, while `createEffect(() => untrack(cb))` reports `deferred`.
Before this change the generator answered `deferred` for all six, so five of
them were claims the probe fails — the same failure class the wrapper-chain fold
exists to remove. The schedule now comes from
`Dialect::tracked_callback_timing`, which carries those source lines, and 2.0
answers differently (its `createEffect` compute *is* eager, via `effect()`'s
unconditional `recompute`).

The three `unestablished…` exports are the fail-closed arm, one per seam that
can publish a row: the primitive-argument branch, the direct-invocation ladder,
and the local-callee forwarding ambient. 1.x `createSignal(fn)` stores the
function as the signal's value and never invokes it, so the dialect states no
schedule, neither `inline` nor `deferred` is true, and the fold refuses instead
of picking one. All three seams treat that refusal as authoritative rather than
falling back to the lexical answer — which is the bug shape, since the lexical
answer is exactly what the fold replaced. The whole `callbacks` domain becomes
the unknown sentinel, the only per-export encoding schema v1 has.

## Why `solid-js/runtime.ts` exists

The three `…ThroughLocal…` exports reach a **different derivation seam** from
the six above, and it is the seam solid-js itself goes through. When the clearing
wrapper is a call the export writes directly, the row comes from the enclosing
chain. When the callback is *forwarded by identity* into a local function
(`untrack(handle)`, not `untrack(() => handle())`), the row comes from that
callee's own summary plus an ambient adjustment — a separate code path, and the
one that produced the `tracked` claim for `onMount`.

Reaching it needs a wrapper that is simultaneously a summary node in the
caller's own file **and** a resolved primitive. `solid_primitive_declaration`
(solid-reactive-ir/src/symbols.rs) is what makes the second possible: it
bootstraps primitive resolution for Solid's own implementation, where no package
import establishes provenance, by accepting a declaration whose path carries an
exact `solid-js` or `@solidjs` component and whose name the dialect declares.
`solid-js/runtime.ts` is therefore named for its path, deliberately, and
`index.ts` re-exports from it. That is not a trick played on the analyzer — it
is the same fact the analyzer uses when the package under analysis *is*
solid-js, and no other fixture shape can produce it.

| Export | claim | before this change |
| --- | --- | --- |
| `mountThroughLocalUntrack` | `deferred` | **`tracked`** — the defect, on solid-js's own shape |
| `memoThroughLocalUntrack` | `inline` | **`deferred`** — the eager twin, wrong through the same seam |
| `unestablishedThroughLocalHelper` | `{"status":"unknown"}` | new: the refusal reached through the forwarding seam |
| `inlineThroughLocalUntrack` | `inline` | `inline` (unchanged) |
| `trackedThroughLocalHelper` | `tracked` | `tracked` (unchanged) |

`trackedThroughLocalHelper` forwards into `runNow`, which invokes its callback
synchronously exactly as `untrack` does and differs *only* in not clearing the
listener. It is the control that shows the answer turns on the clearing fact
rather than on the wrapper being a function call at all.

## What this does not yet prove

Two residues, and both ship an affirmative wrong claim rather than merely losing
a fact. Stated plainly because "recorded" is not the same as "harmless": each
one is a row `contract probe` will fail.

**A package-local transparent wrapper around the real `untrack`.** A schema-v1
`callbacks` row carries the execution word and no clearing column, so once a
local callee's summary crosses the forwarding hop, `runNow`-shaped and
`untrack`-shaped callees are indistinguishable. `function runUntracked<T>(fn:
() => T): T { return untrack(fn); }` used inside `createEffect(() =>
runUntracked(handle))` therefore publishes `tracked`, where the runtime does not
run the callback during the call and subscribes nothing — the honest row is
`deferred`, which is what the reviewed bundled contract states for the identical
`onMount` shape. `trackedThroughLocalHelper` above is the *correct-answer*
control for that spelling (`runNow` does not clear), so nothing in this fixture
pins the wrong case.

**A wrapper the fold cannot classify at all.** `enclosing_callback_chain`
refuses the whole chain on the first position `callback_wrapper_at` cannot
classify, and the row then falls back to the lexical answer this fold exists to
replace. The unclassifiable set is large — `batch`, `startTransition`,
`createComputed`, `onMount`, `catchError`, `children`, `createSelector`,
`createDeferred`, `produce`, `from`, `render`, `hydrate` — so
`untrack(() => batch(() => cb()))` publishes `deferred` where the measured
runtime is `inline`. Pre-existing, unchanged by this fixture, and the miss path
is a positive claim rather than a sentinel.

Both are recorded in docs/precision-backlog.md with their measured shapes.

## Stub faithfulness

`node_modules/solid-js/index.d.ts` transcribes solid-js@1.9.14's
`types/reactive/signal.d.ts`, dropping only inference machinery no claim here
depends on. `solid-js/runtime.ts`'s `untrack` is 1.9.14's body minus the
`ExternalSourceConfig` branch.

Every added signature is *narrower* than the published one, which is the
direction that cannot manufacture a claim: `createEffect`/`createRenderEffect`
drop `EffectFunction`'s seed parameter and its overloads, `createRoot` drops the
optional `detachedOwner`, `createSignal` drops `SignalOptions` and returns a
narrowed setter, and `mergeProps` returns `unknown` instead of `MergeProps<T>`
while accepting the same arguments — nothing here reads its result.
`tsc --noEmit` (TypeScript 5.9.3, `strict`, `moduleResolution: bundler`) is
clean for `index.ts` plus `solid-js/runtime.ts` against **both** this stub and
the real solid-js@1.9.14 typings from `rust/target/tsc-oracle/v1`, so no claim
in this fixture duplicates a TypeScript diagnostic.
