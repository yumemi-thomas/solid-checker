// This file's *path* is the point, twice over.
//
// 1. `solid_primitive_declaration` (solid-reactive-ir/src/symbols.rs)
//    bootstraps primitive resolution for Solid's own implementation, where
//    there is no package import to establish provenance: a declaration whose
//    path carries an exact `solid-js` or `@solidjs` component, and whose name
//    the dialect declares, resolves to that primitive. So `untrack` below is
//    `Primitive::Untrack`.
// 2. It is also a summary node in the same file as its callers, which routes
//    those callers through the *local-callee forwarding* seam
//    (`callback_forwardings`) instead of through the primitive-argument
//    branch. A cross-file helper takes a different path.
//
// Both facts are dialect-neutral: the seam and the path bootstrap outlived the
// Solid 1.x retirement unchanged, which is why this half ports as-is.
import { createEffect, untrack as clearListener } from "solid-js";

let listener: unknown = null;

// A local clearing wrapper: clear the listener, call `fn`, restore.
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
export function runNow<T>(fn: () => T): T {
  return fn();
}

// same-stack / untracked, reached through the forwarding seam rather than the
// primitive-argument branch.
export function untrackedThroughLocalUntrack(handle: () => void): void {
  createEffect(() => untrack(handle), () => {});
}

// same-stack / tracked through the same seam: identical shape, non-clearing
// wrapper. If the seam ever lost the wrapper identity, these two would
// collapse onto one answer.
export function trackedThroughLocalHelper(handle: () => void): void {
  createEffect(() => runNow(handle), () => {});
}

void clearListener;
