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
export function App() {
  const f = (k: string) => () => { __log(k, own()); setW((x) => x + 1); };
  setTimeout(f("timeout"), 0); const iv = setInterval(() => { clearInterval(iv); f("interval")(); }, 0); queueMicrotask(f("micro")); requestAnimationFrame(f("raf")); requestIdleCallback(f("ric"));
  Promise.resolve().then(f("then")); Promise.reject(new Error("x")).catch(f("catch")); Promise.resolve().finally(f("finally"));
  const el = document.createElement("div"); const mo = new MutationObserver(f("mutation")); mo.observe(el, { attributes: true }); el.setAttribute("a", "1");
  const ro = new ResizeObserver(f("resize")); ro.observe(document.body); const io = new IntersectionObserver(f("intersection")); io.observe(document.body); const po = new PerformanceObserver(f("perf")); po.observe({ entryTypes: ["mark"] }); performance.mark("x");
  return <p />;
}
