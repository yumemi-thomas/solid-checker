import { createSignal, createEffect, createRenderEffect, createTrackedEffect, createMemo, createStore, onCleanup, onSettled, getOwner, runWithOwner, createRoot, createContext, useContext, untrack, lazy, children, mapArray, repeat, Show, For, Errored, Loading, flush } from "solid-js";
import { Dynamic, dynamic, clientOnly, render, Portal } from "@solidjs/web";
import type { JSX } from "@solidjs/web";
declare const __log: (...a: any[]) => void;
const [c, setC] = createSignal(1);
const [w, setW] = createSignal(0);
const [on, setOn] = createSignal(false);
const [list, setList] = createSignal([1, 2]);
const [s, setS] = createStore({ a: 1 });
const own = () => (getOwner() as any)?._name ?? String(getOwner());
const btn = (b: HTMLButtonElement) => { (globalThis as any).__b = () => b; };
const click = async () => { (globalThis as any).__b().click(); };
function Inner(p: { get: () => number }) { const v = p.get(); __log("got", v); return <i>{v}</i>; }
export function App() {
  const m = createMemo(() => new Promise<number>((r) => { setTimeout(() => { r(1); }, 5); }));
  return <Loading fallback="x"><Dynamic component={Inner} get={m} /></Loading>;
}
export const after = async () => { await new Promise((r) => setTimeout(r, 30)); __log("dom", document.querySelector("i")?.textContent); };
