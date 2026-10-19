import { createSignal, createMemo, createEffect, createRoot, runWithOwner, getOwner, onSettled, onCleanup, untrack, createTrackedEffect, children } from "solid-js";
function now(f: () => void) {
  f();
}
export function C() {
  const [count] = createSignal(0);
  createRoot(() => console.log(count()));
  return <main />;
}
