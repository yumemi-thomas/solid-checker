import { createSignal, createMemo, createEffect, createRoot, getOwner, runWithOwner, onCleanup, reconcile, createStore, untrack, Loading, flush } from "solid-js";
const log = (...a: any[]) => { (window as any).__log?.(...a); };
const own = () => (getOwner() ? "owner" : "no-owner");
const [c, setC] = createSignal(1);
const [c2, setC2] = createSignal(1);
const [st, setSt] = createStore({ x: 1, items: [{ id: 1 }] });
const a = createMemo(async () => { await new Promise((r) => setTimeout(r, 40)); return 7; });
export function App() {
  (window as any).__go = () => { log("ran", own()); try { createRoot(() => a()); } catch (e) { log("threw", String(e).slice(0, 40)); } };
  return <i>app</i>;
}
export async function after() { (window as any).__go(); await new Promise((r) => setTimeout(r, 20)); }
