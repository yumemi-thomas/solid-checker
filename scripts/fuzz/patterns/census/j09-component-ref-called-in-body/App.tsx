import { createSignal, createMemo, createEffect, createRoot, getOwner, onCleanup, Show, For, lazy, dynamicComponent, mapArray, reconcile, createStore, untrack } from "solid-js";
import { Dynamic, clientOnly } from "@solidjs/web";
const log = (...a: any[]) => (window as any).__log?.(...a);
const own = () => (getOwner() ? "owner" : "no-owner");
const [c, setC] = createSignal(1);
const [c2, setC2] = createSignal(1);
const [w, setW] = createSignal(0);
const [on, setOn] = createSignal(true);
const [xs, setXs] = createSignal([{ id: 1 }, { id: 2 }]);
const misuseEffect = (tag: string) => { log(tag, own()); createEffect(() => c(), () => {}); };
function C(props: { ref?: (el: HTMLElement) => void }) { log("C", "calls ref"); props.ref?.(document.createElement("i")); return <i>c</i>; }
export function App() {

  return <C ref={(el) => { log("ref", own()); setW(w() + 1); log("ref", "write ok"); }} />;
}
