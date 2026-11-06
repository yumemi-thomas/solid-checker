import { createSignal, createMemo, createEffect, createRoot, getOwner, onCleanup, Show, For, lazy, dynamicComponent, mapArray, reconcile, createStore, untrack } from "solid-js";
import { Dynamic, clientOnly } from "@solidjs/web";
const log = (...a: any[]) => (window as any).__log?.(...a);
const own = () => (getOwner() ? "owner" : "no-owner");
const [c, setC] = createSignal(1);
const [c2, setC2] = createSignal(1);
const [w, setW] = createSignal(0);
const [on, setOn] = createSignal(true);
const [xs, setXs] = createSignal([{ id: 1 }, { id: 2 }]);
const misuseEffect = (tag: string) => { log(tag, own()); createEffect(() => c(), () => {}); };
export function App() {
  let el!: HTMLDivElement;
  queueMicrotask(() => new MutationObserver(() => misuseEffect("mo")).observe(el, { childList: true }));
  return <div class="t" ref={el}>m</div>;
}
export async function after() {
  document.querySelector(".t")!.append("x"); await new Promise((r) => setTimeout(r, 20));
}