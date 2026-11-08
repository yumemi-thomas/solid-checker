import { createSignal, createMemo, createEffect, createRoot, getOwner, onCleanup, Show, For, lazy, dynamicComponent, mapArray, reconcile, createStore, untrack, NotReadyError, Loading } from "solid-js";
import { Dynamic, clientOnly, dynamic } from "@solidjs/web";
const log = (...a: any[]) => { (window as any).__log?.(...a); };
const own = () => (getOwner() ? "owner" : "no-owner");
const [c, setC] = createSignal(1);
const [c2, setC2] = createSignal(1);
const [w, setW] = createSignal(0);
const [on, setOn] = createSignal(true);
let runs = 0;
function HiddenR(props: { v: number }) { log("HiddenR", "body " + own() + " v=" + props.v); return <i>h</i>; }
export function App() {

  return <Dynamic component={HiddenR} v={w()} />;
}
export async function after() {
  setW(5); await new Promise((r) => setTimeout(r, 30));
}