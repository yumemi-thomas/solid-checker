import { createSignal, createEffect, createRenderEffect, createMemo, onCleanup, getOwner, runWithOwner, createRoot, untrack, Show, For, Errored, flush } from "solid-js";
import { Dynamic } from "@solidjs/web";
declare const __log: (...a: any[]) => void;
const [c, setC] = createSignal(1);
const [w, setW] = createSignal(0);
const [on, setOn] = createSignal(false);
const own = () => (getOwner() as any)?._name ?? String(getOwner());
export function App() {
  let b!: HTMLButtonElement;
  (globalThis as any).__b = () => b;
  const el = <button ref={b}>{w()}</button> as HTMLButtonElement;
  el.addEventListener("click", () => { c(); setW(1); __log("listener owner", own()); });
  return el;
}
export async function after() { (globalThis as any).__b().click(); }
