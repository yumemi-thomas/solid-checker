import { createMemo, createEffect } from "solid-js";

// ADR 0183: a caller's function handed, unconditionally, to an eager owned
// computation the export's own body creates runs during every call as that
// computation's compute.
export function derive(fn) {
  createMemo(fn);
}

// The memo is created only when `flag` holds: the owner is stated, the lower
// bound is not.
export function deriveMaybe(flag, fn) {
  if (flag) createMemo(fn);
}

// An effect's compute runs during the call; its effect function is queued.
export function watch(compute, effect) {
  createEffect(compute, effect);
}

// `fn` runs inside a compute the export writes itself: an enclosing callback
// position, so the slot's own owner answer is not the row's.
export function deriveWrapped(fn) {
  createMemo(() => fn());
}
