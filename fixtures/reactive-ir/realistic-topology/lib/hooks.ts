import { createEffect, onCleanup } from "solid-js";
import { count } from "./store";

// The helper whose only read is sealed inside a tracked callback. Calling it
// performs no read, so no caller may be charged for one -- the shape that
// produced a *false violation* until the interprocedural summary learned to
// respect a callback's execution.
export function watchCount() {
  createEffect(() => count(), (value) => sink(value));
  onCleanup(() => sink(-1));
}

// The contrast: the read is in the helper's own body, so calling it does
// perform the read and the caller is charged.
export function readCountNow() {
  return count();
}

declare function sink(value: number): void;
