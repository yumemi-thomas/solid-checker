import { createSignal, createEffect, createRenderEffect, createMemo, onCleanup, getOwner, runWithOwner, createRoot, untrack, Show, For, Errored, flush } from "solid-js";
import { Dynamic } from "@solidjs/web";
declare const __log: (...a: any[]) => void;
const [c, setC] = createSignal(1);
const [w, setW] = createSignal(0);
const [on, setOn] = createSignal(false);
const own = () => (getOwner() as any)?._name ?? String(getOwner());
export function App() {
  createRenderEffect(() => c(), (v) => { __log("rfx apply owner", own()); setW(v); });
  return <p>{w()}</p>;
}
