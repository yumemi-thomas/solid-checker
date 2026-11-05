import { createSignal, createEffect, createRenderEffect, createMemo, onCleanup, getOwner, runWithOwner, createRoot, untrack, Show, For, Errored, flush } from "solid-js";
import { Dynamic } from "@solidjs/web";
declare const __log: (...a: any[]) => void;
const [c, setC] = createSignal(1);
const [w, setW] = createSignal(0);
const [on, setOn] = createSignal(false);
const own = () => (getOwner() as any)?._name ?? String(getOwner());
function h(data: number) { c(); setW(data); __log("tuple handler owner", own()); }
export function App() {
  let b!: HTMLButtonElement;
  (globalThis as any).__b = () => b;
  return <button ref={b} onClick={[h, 1]}>{w()}</button>;
}
export async function after() { (globalThis as any).__b().click(); }
