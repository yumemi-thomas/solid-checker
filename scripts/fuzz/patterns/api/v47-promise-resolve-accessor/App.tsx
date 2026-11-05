import { createSignal, createMemo, createEffect, createRoot, getOwner, runWithOwner, onCleanup, Show, For, lazy, mapArray, repeat, reconcile, createStore, untrack, flush, Loading, omit, merge, NotReadyError, resolve, until, children, useContext, createContext, onSettled } from "solid-js";
import { Dynamic, clientOnly, dynamic } from "@solidjs/web";
const log = (...a: any[]) => { (window as any).__log?.(...a); };
const own = () => (getOwner() ? "owner" : "no-owner");
const [c, setC] = createSignal(1);
const [c2, setC2] = createSignal(1);
const [st, setSt] = createStore({ x: 1, o: { y: 2 }, items: [{ id: 1 }] });
const unk = (): any => null;
export function App() {
  const p = Promise.resolve(c);
  p.then((f) => { log("t", own()); createEffect(() => f(), () => {}); });
  return <i>app</i>;
}
