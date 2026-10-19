import "./runtime";
declare const opaque: { run: () => void };
declare function makeOpaque(): { run: () => void };
declare const partial: { run?: () => void; other?: number };
declare function opaqueFn(o: { run: () => void }): void;
declare function opaqueCb(f: () => void): void;
import { createSignal, createMemo, createEffect, createRoot, runWithOwner, getOwner, onSettled, createTrackedEffect } from "solid-js";
import * as Solid from "solid-js";
declare const p: Promise<number>;
function now(f: () => void) {
  f();
}
export function C() {
  const [count] = createSignal(0);
  createRoot(() => console.log(count()));
  return <main />;
}
