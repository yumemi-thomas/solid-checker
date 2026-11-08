import { createSignal, createMemo, createEffect, createRoot, getOwner, onCleanup, Show, For, lazy, dynamicComponent, mapArray, reconcile, createStore, untrack } from "solid-js";
import { Dynamic, clientOnly } from "@solidjs/web";
const log = (...a: any[]) => { (window as any).__log?.(...a); };
const own = () => (getOwner() ? "owner" : "no-owner");
const [c, setC] = createSignal(1);
const [c2, setC2] = createSignal(1);
const [w, setW] = createSignal(0);
const [on, setOn] = createSignal(true);
let runs = 0;
function Hidden() { runs += 1; log("Hidden", "body " + own() + " runs=" + runs + " w=" + w()); return <i>h</i>; }
export function App() {
  const L = lazy(() => Promise.resolve({ default: Hidden }));
  return <L />;
}
export async function after() {
  setW(5); await new Promise((r) => setTimeout(r, 30));
}