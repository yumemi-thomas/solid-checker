import "./runtime";
declare const opaque: { run: () => void };
declare function makeOpaque(): { run: () => void };
declare const partial: { run?: () => void; other?: number };
declare function opaqueFn(o: { run: () => void }): void;
declare function opaqueCb(f: () => void): void;
import { createSignal, createMemo, createEffect, onCleanup, untrack } from "solid-js";
function mix(cb: () => void) {
  const m = createMemo(() => (cb(), 1));
  untrack(cb);
  return m;
}
export function C() {
  const m = mix(() => onCleanup(() => undefined));
  return <main>{m()}</main>;
}
