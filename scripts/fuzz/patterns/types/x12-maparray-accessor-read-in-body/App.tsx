import { createSignal, createEffect, createMemo, onCleanup, getOwner, createRoot, children, mapArray, repeat, Show, For, flush } from "solid-js";
import { Dynamic } from "@solidjs/web";
import type { JSX } from "@solidjs/web";
declare const __log: (...a: any[]) => void;
const [c, setC] = createSignal(1);
const [w, setW] = createSignal(0);
const [on, setOn] = createSignal(false);
const [list, setList] = createSignal([1, 2]);
const own = () => (getOwner() as any)?._name ?? String(getOwner());
const btn = (b: HTMLButtonElement) => { (globalThis as any).__b = () => b; };
const click = async () => { (globalThis as any).__b().click(); };
export function App() {
  const mapped = mapArray(() => list(), (x) => x * 2);
  const v = mapped();
  return <p>{v.length}</p>;
}
