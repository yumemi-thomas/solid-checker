import { createSignal, createMemo, createEffect, createRoot, getOwner, runWithOwner, onCleanup, reconcile, createStore, untrack, Loading, flush } from "solid-js";
const log = (...a: any[]) => { (window as any).__log?.(...a); };
const own = () => (getOwner() ? "owner" : "no-owner");
const [c, setC] = createSignal(1);
const [c2, setC2] = createSignal(1);
const [st, setSt] = createStore({ x: 1, items: [{ id: 1 }] });
export function App() {
  setTimeout(() => { log("ran", own()); setSt(reconcile({ x: 1, items: [{ id: 2 }] }, (it: any) => { setC(3); return it.id; })); }, 0);
  return <i>app</i>;
}
