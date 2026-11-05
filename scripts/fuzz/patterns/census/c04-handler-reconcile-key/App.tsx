import { createSignal, createMemo, createEffect, createRoot, getOwner, onCleanup, Show, For, lazy, dynamicComponent, mapArray, reconcile, createStore, untrack } from "solid-js";
import { Dynamic, clientOnly } from "@solidjs/web";
const log = (...a: any[]) => { (window as any).__log?.(...a); };
const own = () => (getOwner() ? "owner" : "no-owner");
const [c, setC] = createSignal(1);
const [c2, setC2] = createSignal(1);
const [w, setW] = createSignal(0);
const [on, setOn] = createSignal(true);
let runs = 0;
export function App() {
  const [st, setSt] = createStore({ items: [{ id: 1 }] });
  return <button id="b" onClick={() => setSt(reconcile({ items: [{ id: 2 }] }, (it: any) => (log("key", own()), w(), it.id)))}>{st.items.length}</button>;
}
export async function after() {
  (document.getElementById("b") as HTMLButtonElement).click();
}