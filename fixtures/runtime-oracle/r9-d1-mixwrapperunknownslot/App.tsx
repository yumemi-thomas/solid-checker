import "./runtime";
declare const opaque: { run: () => void };
declare function makeOpaque(): { run: () => void };
declare const partial: { run?: () => void; other?: number };
declare function opaqueFn(o: { run: () => void }): void;
declare function opaqueCb(f: () => void): void;
import { createSignal, createMemo, createEffect, createRoot, runWithOwner, getOwner, onSettled, onCleanup, untrack, createTrackedEffect, children } from "solid-js";
function now(f: () => void) {
  f();
}
declare function opaqueSlot(f: () => void): void;
function use(cb: () => void) {
  const m = createMemo(() => (cb(), 1));
  opaqueSlot(cb);
  return m;
}
export function C() {
  const [count] = createSignal(0);
  const m = use(() => console.log(count()));
  return <main>{m()}</main>;
}
