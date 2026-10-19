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
  let reached = false;
  setTimeout(() => { document.querySelector("#oracle-path")!.textContent = String(reached); }, 10);
  const [count] = createSignal(0);
  void (async () => { if (c) await p; console.log(count()); reached = true; })();
  return <main id="oracle-path">waiting</main>;
}
