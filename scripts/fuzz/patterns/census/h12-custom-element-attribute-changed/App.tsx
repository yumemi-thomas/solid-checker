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
class XB extends HTMLElement { static observedAttributes = ["v"]; attributeChangedCallback() { log("attr", own()); const v = c2(); log("attr", "read c2=" + v); setW(w() + 1); log("attr", "write ok"); } }
customElements.define("x-b", XB);
declare module "solid-js" { namespace JSX { interface IntrinsicElements { "x-a": any; "x-b": any; } } }
export function App() {
  createEffect(() => c2(), (v) => log("c2-dep", v));
  return <x-b v={String(c())}>x</x-b>;
}
export async function after() {
  setC(2); await new Promise((r) => setTimeout(r, 10)); setC2(2);
}