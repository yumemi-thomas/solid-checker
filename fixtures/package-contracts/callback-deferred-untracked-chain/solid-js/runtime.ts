// This file's *path* is the point, twice over.
//
// 1. `solid_primitive_declaration` (solid-reactive-ir/src/symbols.rs) bootstraps
//    primitive resolution for Solid's own implementation, where there is no
//    package import to establish provenance: a declaration whose path carries
//    an exact `solid-js` or `@solidjs` component, and whose name the dialect
//    declares, resolves to that primitive. So `untrack` below is
//    `Primitive::Untrack`.
// 2. It is also a summary node in the same file as its callers, which is what
//    routes those callers through the *local-callee forwarding* seam
//    (`callback_forwardings`) instead of through the primitive-argument branch.
//    A cross-file helper takes a different path, and this seam is the one
//    solid-js's own `dist/solid.js` uses -- `untrack`, `createEffect` and
//    `onMount` all live in one file there.
//
// Together those two facts make this the only shape a fixture can use to reach
// the derivation that published `tracked` for solid-js's `onMount`.
import { createEffect, createMemo, createSignal, untrack as clearListener } from "solid-js";

let listener: unknown = null;

// 1.9.14's body minus the `ExternalSourceConfig` branch: clear the listener,
// call `fn`, restore.
export function untrack<T>(fn: () => T): T {
  const previous = listener;
  listener = null;
  try {
    return fn();
  } finally {
    listener = previous;
  }
}

// The control wrapper: invokes its callback synchronously exactly as `untrack`
// does, and differs only in not clearing the listener. Deliberately not a
// dialect name, so it stays an ordinary local function.
export function runNow(fn: () => void): void {
  fn();
}

// Positive: `onMount(fn) { createEffect(() => untrack(fn)) }`, solid-js
// 1.9.14 `dist/solid.js:485-487`. The clearing wrapper stops the enclosing
// effect from tracking the callback; the effect still schedules it, so it has
// not run when this function returns.
export function mountThroughLocalUntrack(handle: () => void): void {
  createEffect(() => untrack(handle));
}

// Positive: the eager twin of `mountThroughLocalUntrack`, through the same
// forwarding seam. `createMemo` runs its computation during the call, so the
// clearing wrapper's `inline` survives instead of becoming `deferred`. This is
// the pair that shows the seam reads the wrapper's *schedule* from the dialect
// rather than reading "tracked" as "later".
export function memoThroughLocalUntrack(handle: () => number): () => number {
  return createMemo(() => untrack(handle));
}

// Negative: the same clearing wrapper with nothing deferring above it runs the
// callback during the call.
export function inlineThroughLocalUntrack(handle: () => void): void {
  untrack(handle);
}

// Positive sentinel, third seam: the callback is forwarded into a *local*
// callee (`runNow`), so the row comes from that callee's summary plus an
// ambient adjustment -- and the adjustment's chain contains a tracked wrapper
// with no established schedule. The callee's `inline` row cannot be restated in
// export-relative terms, so it is dropped and the sentinel opens rather than
// the row being republished verbatim.
export function unestablishedThroughLocalHelper(handle: () => void): void {
  createSignal(() => clearListener(() => runNow(handle)));
}

// Negative: swap the clearing wrapper for the transparent one and the enclosing
// effect really does track the callback. This is the pair that shows the answer
// turns on the clearing fact and not on the wrapper being a function call.
export function trackedThroughLocalHelper(handle: () => void): void {
  createEffect(() => runNow(handle));
}
