import { createSignal, createMemo, createEffect, createRoot, runWithOwner, getOwner, onSettled, onCleanup, untrack, createTrackedEffect, children } from "solid-js";
function now(f: () => void) {
  f();
}
function use(cb: () => number) {
  const [s] = createSignal(cb);
  return s;
}
export function C() {
  const [count] = createSignal(0);
  const s = use(() => count());
  return <main>{s()}</main>;
}
