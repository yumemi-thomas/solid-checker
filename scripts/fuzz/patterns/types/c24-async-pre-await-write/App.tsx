import { createSignal, createEffect, createRenderEffect, createMemo, onCleanup, getOwner, runWithOwner, createRoot, untrack, Show, For, Errored, flush } from "solid-js";
import { Dynamic } from "@solidjs/web";
declare const __log: (...a: any[]) => void;
const [c, setC] = createSignal(1);
const [w, setW] = createSignal(0);
const [on, setOn] = createSignal(false);
const own = () => (getOwner() as any)?._name ?? String(getOwner());
async function go() { setW(1); await 0; setW(2); }
export function App() {
  go().catch((e) => __log("go rejected", e.message));
  return <p>{w()}</p>;
}
