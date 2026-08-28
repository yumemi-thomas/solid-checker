import {
  createEffect,
  createMemo,
  createRenderEffect,
  createRoot,
  createSignal,
  mergeProps,
  onCleanup,
  untrack
} from "solid-js";

// The forwarding-seam half of this fixture; see solid-js/runtime.ts for why it
// has to live in a separately-pathed module.
export {
  inlineThroughLocalUntrack,
  memoThroughLocalUntrack,
  mountThroughLocalUntrack,
  trackedThroughLocalHelper,
  unestablishedThroughLocalHelper
} from "./solid-js/runtime.js";

// Positive: solid-js 1.9.14's own `onMount`, byte for byte
// (`dist/solid.js:485-487`). The `untrack` clears the listener, so the callback
// is not tracked; the `createEffect` still schedules it, so it has not run when
// `mountShape` returns. The normalized operation is queued and untracked,
// matching the reviewed `onMount` authority and runtime behavior.
export function mountShape(handle: () => void): void {
  createEffect(() => untrack(handle));
}

// Positive: the same chain with an explicit arrow, so the claim does not depend
// on the callback being forwarded by identity.
export function mountShapeArrow(handle: () => void): void {
  createEffect(() => untrack(() => handle()));
}

// Positive: the clearing wrapper inside a genuinely later one. Deferral is
// sticky -- nothing outside can make the callback run during the call.
export function cleanupShape(handle: () => void): void {
  onCleanup(() => untrack(() => handle()));
}

// Negative, and the one that makes the rule a rule rather than "untrack
// anywhere means not tracked": order decides. The memo subscribes what runs
// inside it, and the surrounding `untrack` cannot undo that subscription, so
// the callback stays tracked.
export function memoInsideUntrack(handle: () => number): () => number {
  return untrack(() => createMemo(() => handle()));
}

// Negative: drop the clearing wrapper and the tracked claim comes back.
export function trackedShape(handle: () => void): void {
  createEffect(() => handle());
}

// Negative: drop the deferring wrapper and the callback runs during the call.
export function inlineShape(handle: () => void): void {
  untrack(() => handle());
}

// Positive, and the shape that shows `Tracked` alone cannot answer the
// schedule: the *same* chain as `mountShape` with an eager tracked wrapper.
// 1.x `createMemo` calls `updateComputation(c)` before it returns
// (`dist/solid.js:244-256`), so `handle` has already run when `memoShape`
// returns. Measured against solid-js@1.9.14 under `--conditions browser`:
// `ranDuringCall`, no re-run, i.e. `inline`.
export function memoShape(handle: () => number): () => number {
  return createMemo(() => untrack(handle));
}

// Positive: the second eager 1.x primitive (`dist/solid.js:218-221`, the same
// `updateComputation` line), with `createRoot` as the clearing wrapper instead
// of `untrack`. Neither half of the pair is what makes the answer `inline`.
export function renderEffectShape(handle: () => void): void {
  createRenderEffect(() => createRoot(() => handle()));
}

// Positive: the component-defaults idiom, which is the largest single group in
// the corpus measurement. `mergeProps` wraps every function-valued source in a
// memo (`dist/solid.js:1329`), so it is eager for exactly the same reason
// `createMemo` is -- at every argument index, which is why the dialect answers
// per argument rather than per primitive.
export function mergePropsShape(handle: () => number): void {
  mergeProps({ a: 1 }, () => untrack(handle));
}

// Positive open leaf: a tracked wrapper this dialect states no schedule for.
// 1.x `createSignal(fn)` stores the function as the signal's value and never
// invokes it (`Solid1x::stores_function_argument_as_value`), so neither
// same-stack nor queued is proven and the fold refuses rather than picking one.
// The callback domain remains open in the proposal.
export function unestablishedScheduleShape(handle: () => number): void {
  createSignal(() => untrack(handle));
}

// Positive open leaf, second seam: the same chain with the callback *invoked*
// rather than forwarded by identity, which is a different rung of the ladder.
// A refusal there must not fall through to the lexical answer either.
export function unestablishedDirectShape(handle: () => number): void {
  createSignal(() => untrack(() => handle()));
}
