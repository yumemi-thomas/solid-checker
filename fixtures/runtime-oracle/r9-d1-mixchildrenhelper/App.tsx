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
function use(cb: () => import("@solidjs/web").JSX.Element) {
  const c = children(cb);
  untrack(cb);
  return c;
}
export function C() {
  const [count] = createSignal(0);
  const c = use(() => count());
  return <main>{String(c())}</main>;
}
