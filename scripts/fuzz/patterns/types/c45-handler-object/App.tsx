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
  return <button ref={b} onClick={{ handleEvent(e: MouseEvent) { c(); setW(1); __log("handleEvent owner", own()); } }}>{w()}</button>;
}
export async function after() { (globalThis as any).__b().click(); }
