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
const flag = () => document.title === "";
export function App() {
  const [x, setX] = createSignal(0, { ownedWrite: flag() }); setX(1); __log("ok");
  return <p>{x()}</p>;
}
