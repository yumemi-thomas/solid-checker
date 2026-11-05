import { createSignal, createMemo, createEffect, createRoot, getOwner, runWithOwner, onCleanup, reconcile, createStore, untrack, Loading, flush } from "solid-js";
const log = (...a: any[]) => { (window as any).__log?.(...a); };
const own = () => (getOwner() ? "owner" : "no-owner");
const [c, setC] = createSignal(1);
const [c2, setC2] = createSignal(1);
const [st, setSt] = createStore({ x: 1, items: [{ id: 1 }] });
const a = createMemo(async () => { await new Promise((r) => setTimeout(r, 40)); return 7; });
export function App() {
  const go = () => { log("ran", own()); createRoot(() => a()); }; setTimeout(go, 0);
  return <i>app</i>;
}
