import { createSignal, createEffect, createRenderEffect, createMemo, onCleanup, onSettled, getOwner, createOwner, runWithOwner, createRoot, untrack, lazy, Show, For, Errored, flush } from "solid-js";
import { Dynamic, Portal } from "@solidjs/web";
declare const __log: (...a: any[]) => void;
const [c, setC] = createSignal(1);
const [w, setW] = createSignal(0);
const [on, setOn] = createSignal(false);
const own = () => (getOwner() as any)?._name ?? String(getOwner());
const btn = (b: HTMLButtonElement) => { (globalThis as any).__b = () => b; };
const click = async () => { (globalThis as any).__b().click(); };
export function App() {
  runWithOwner(null, () => { __log("rwo null owner", own()); setW(1); c(); });
  return <p>{w()}</p>;
}
