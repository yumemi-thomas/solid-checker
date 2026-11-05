import { createSignal, createMemo, createEffect, createRoot, getOwner, runWithOwner, onCleanup, reconcile, createStore, untrack, Loading, flush } from "solid-js";
const log = (...a: any[]) => { (window as any).__log?.(...a); };
const own = () => (getOwner() ? "owner" : "no-owner");
const [c, setC] = createSignal(1);
const [c2, setC2] = createSignal(1);
const [st, setSt] = createStore({ x: 1, items: [{ id: 1 }] });
export function App() {

  createEffect(() => c2(), () => { log("apply", own()); const v = Object.entries({ c })[0][1](); log("apply", "read " + String(v)); });
  return <i>app</i>;
}
