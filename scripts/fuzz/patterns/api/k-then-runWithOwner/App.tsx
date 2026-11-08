import { createSignal, createMemo, createEffect, createRoot, getOwner, runWithOwner, onCleanup, Show, For, lazy, dynamicComponent, mapArray, repeat, reconcile, createStore, untrack, flush, Loading, omit, merge, createProjection, createOptimistic, createReaction, onSettled, latest, isPending, action } from "solid-js";
import { Dynamic, clientOnly, dynamic } from "@solidjs/web";
const log = (...a: any[]) => { (window as any).__log?.(...a); };
const own = () => (getOwner() ? "owner" : "no-owner");
const [w, setW] = createSignal(0);
const [on, setOn] = createSignal(false);
const [st, setSt] = createStore({ x: 1, items: [{ id: 1 }] });
let runs = 0;
function Hidden() { runs += 1; log("Hidden", "body " + own() + " runs=" + runs + " w=" + w()); return <i>h</i>; }
export function App() {
  Promise.resolve().then(() => { log("ran", own()); runWithOwner(null, () => w()); });
  return <i>app</i>;
}
export async function after() { (window as any).__go?.(); await new Promise((r) => setTimeout(r, 20)); }
