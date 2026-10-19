import "./runtime";
declare const opaque: { run: () => void };
declare function makeOpaque(): { run: () => void };
declare const partial: { run?: () => void; other?: number };
declare function opaqueFn(o: { run: () => void }): void;
declare function opaqueCb(f: () => void): void;
import { createSignal, createMemo, createEffect } from "solid-js";
declare const p: Promise<number>;
declare const c: boolean;
export function C() {
  const [count] = createSignal(0);
  void p.then(() => console.log(count()));
  return <main />;
}
