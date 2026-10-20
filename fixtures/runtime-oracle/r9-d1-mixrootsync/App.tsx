import { createSignal, createMemo, createEffect, createRoot, runWithOwner, getOwner, onSettled, onCleanup, untrack, createTrackedEffect, children } from "solid-js";
function now(f: () => void) {
  f();
}
function use(cb: () => void) {
  const m = createMemo(() => (cb(), 1));
  createRoot(() => cb());
  return m;
}
export function C() {
  const [count] = createSignal(0);
  const m = use(() => console.log(count()));
  return <main>{m()}</main>;
}
