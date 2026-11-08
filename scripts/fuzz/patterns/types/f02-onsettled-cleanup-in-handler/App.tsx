import { createSignal, createEffect, createMemo, createTrackedEffect, onCleanup, onSettled, onFlush, getOwner, createRoot, children, createComponent, Show, For, Reveal, flush } from "solid-js";
import type { JSX } from "solid-js";
declare const __log: (...a: any[]) => void;
const [c, setC] = createSignal(1);
const [w, setW] = createSignal(0);
const [on, setOn] = createSignal(false);
const own = () => (getOwner() as any)?._name ?? String(getOwner());
const btn = (b: HTMLButtonElement) => { (globalThis as any).__b = () => b; };
const click = async () => { (globalThis as any).__b().click(); };
export function App() {
  return <button ref={btn} onClick={() => { onSettled(() => { __log("settled"); return () => __log("never"); }); }} />;
}
export const after = click;
