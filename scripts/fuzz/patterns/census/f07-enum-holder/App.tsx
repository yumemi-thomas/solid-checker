import { createSignal, createMemo, createEffect, createRoot, getOwner, onCleanup, Show, For, lazy, dynamicComponent, mapArray, reconcile, createStore, untrack, NotReadyError, Loading } from "solid-js";
import { Dynamic, clientOnly, dynamic } from "@solidjs/web";
const log = (...a: any[]) => { (window as any).__log?.(...a); };
const own = () => (getOwner() ? "owner" : "no-owner");
const [c, setC] = createSignal(1);
const [c2, setC2] = createSignal(1);
const [w, setW] = createSignal(0);
const [on, setOn] = createSignal(true);
let runs = 0;
const enum E { A = 1 }
const holder = { [E.A]: c };
export function App() {

  createEffect(() => c2(), () => { log("apply", own()); const v = holder[E.A](); log("apply", "read " + String(v)); });
  return <i>app</i>;
}
