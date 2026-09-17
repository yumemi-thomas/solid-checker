import {
  createEffect,
  createMemo,
  createSignal,
  createTrackedEffect,
  onCleanup,
  untrack
} from "solid-js";

// The forwarding-seam half; see solid-js/runtime.ts for why it has to live in
// a separately-pathed module.
export {
  trackedThroughLocalHelper,
  untrackedThroughLocalUntrack
} from "./solid-js/runtime.js";

// Schedule and tracking are independent axes, and these five exports are the
// grid. Every one is `createEffect`, `createMemo` or `untrack` in some
// arrangement, so nothing here varies except which wrapper the callback sits
// under.

// same-stack / tracked. 2.0's `createEffect` runs its *compute* during the
// creating call, so `handle` has already run when this returns -- and the
// compute is a tracking scope, so what it reads subscribes.
export function trackedShape(handle: () => void): void {
  createEffect(() => handle(), () => {});
}

// same-stack / untracked. The clearing wrapper moves the *tracking* axis and
// leaves the schedule alone. This is the pair that makes the two axes
// separable: it differs from `trackedShape` in one word, and only one.
export function mountShape(handle: () => void): void {
  createEffect(() => untrack(handle), () => {});
}

// queued / tracked -- the one 2.0 primitive that really does defer.
// `createTrackedEffect` builds its computed with `lazy: true` and enqueues it,
// so nothing runs before this returns. A rule that answered `queued` for every
// tracked callback would be right here and wrong in all four rows above.
export function deferredTrackedShape(handle: () => void): void {
  createTrackedEffect(() => handle());
}

// queued / untracked. `onCleanup` really does run its callback later, and the
// clearing wrapper inside it does not make it earlier -- deferral is sticky.
export function cleanupShape(handle: () => void): void {
  onCleanup(() => untrack(() => handle()));
}

// same-stack / untracked with no tracked wrapper at all, as the control for
// the two `untrack` rows above.
export function inlineShape(handle: () => void): void {
  untrack(() => handle());
}

// Negative, and the one that makes the rule a rule rather than "untrack
// anywhere means not tracked": order decides. The memo subscribes what runs
// inside it, and the surrounding `untrack` cannot undo that subscription, so
// the callback stays tracked. Its schedule is still same-stack, because
// `createMemo` computes during the creating call.
export function memoInsideUntrack(handle: () => number): () => number {
  return untrack(() => createMemo(() => handle()));
}

// The eager twin, with the wrappers the other way round: the clearing wrapper
// is *inside*, so the callback is untracked while the memo still computes it
// during the call.
export function memoShape(handle: () => number): () => number {
  return createMemo(() => untrack(handle));
}

// 2.0's derived `createSignal` selects on `typeof first === "function"`, so
// this is a compute slot and not a stored value -- the 1.x reading, where
// `createSignal(fn)` kept the function and never invoked it, does not apply.
export function derivedSignalShape(handle: () => number): void {
  createSignal(() => untrack(handle));
}
