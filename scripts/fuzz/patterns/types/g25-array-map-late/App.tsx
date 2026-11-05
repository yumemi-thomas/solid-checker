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
class Lazy<T> extends Array<T> {
  override map<U>(cb: (v: T, i: number, a: T[]) => U): U[] { const out: U[] = []; setTimeout(() => { super.forEach((v, i, a) => { out.push(cb(v, i, a)); }); }, 0); return out; }
}
function take(xs: number[]) { return xs.map(() => { __log("ran", c()); return 0; }); }
export function App() {
  const a = new Lazy<number>(); a.push(1);
  take(a);
  return <p />;
}
