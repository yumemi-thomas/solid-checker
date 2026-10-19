import { createSignal, createMemo, createEffect, createRoot, runWithOwner, getOwner, onSettled, onCleanup, untrack, createTrackedEffect, children } from "solid-js";
function now(f: () => void) {
  f();
}
function use(cb: () => number) {
  const m = createMemo(cb);
  const first = m();
  return () => m() + first;
}
export function C() {
  const [count] = createSignal(0);
  const r = use(() => count());
  return <main>{r()}</main>;
}
