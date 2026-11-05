import { createSignal, createMemo, createEffect, createRoot, getOwner, onCleanup, Show, For, lazy, dynamicComponent, mapArray, reconcile, createStore, untrack, NotReadyError, Loading } from "solid-js";
import { Dynamic, clientOnly, dynamic } from "@solidjs/web";
const log = (...a: any[]) => { (window as any).__log?.(...a); };
const own = () => (getOwner() ? "owner" : "no-owner");
const [c, setC] = createSignal(1);
const [c2, setC2] = createSignal(1);
const [w, setW] = createSignal(0);
const [on, setOn] = createSignal(true);
let runs = 0;
export function App() {
  const [xs, setXs] = createSignal([{ id: 1 }]);
  (window as any).__go = () => setXs([{ id: 2 }]);
  return <For each={xs()} keyed={(x) => { log("key", own()); setW(w() + 1); log("key", "write ok"); return x.id; }}>{(x) => <i>{x().id}</i>}</For>;
}
export async function after() {
  (window as any).__go();
}