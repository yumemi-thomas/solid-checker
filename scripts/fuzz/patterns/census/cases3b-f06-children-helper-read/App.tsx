import { createSignal, createMemo, createEffect, createRoot, getOwner, onCleanup, Show, For, lazy, dynamicComponent, mapArray, reconcile, createStore, untrack, NotReadyError, Loading, children } from "solid-js";
import { Dynamic, clientOnly, dynamic } from "@solidjs/web";
const log = (...a: any[]) => { (window as any).__log?.(...a); };
const own = () => (getOwner() ? "owner" : "no-owner");
const [c, setC] = createSignal(1);
const [c2, setC2] = createSignal(1);
const [w, setW] = createSignal(0);
const [on, setOn] = createSignal(true);
let runs = 0;
function Kid(props: { children: any }) { const k = children(() => props.children); createEffect(() => c2(), () => { log("apply", own()); log("apply", "kids " + String(k())); }); return <>{k()}</>; }
export function App() {

  return <Kid>{c()}</Kid>;
}
