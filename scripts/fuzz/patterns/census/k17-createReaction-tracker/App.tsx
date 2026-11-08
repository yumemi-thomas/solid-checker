import { createSignal, createMemo, createEffect, createRoot, getOwner, onCleanup, Show, For, mapArray, repeat, reconcile, createStore, untrack, runWithOwner, createReaction, children } from "solid-js";
const log = (...a: any[]) => { (window as any).__log?.(...a); };
const own = () => (getOwner() ? "owner" : "no-owner");
const [c, setC] = createSignal(1);
const [c2, setC2] = createSignal(1);
const [w, setW] = createSignal(0);
const [xs, setXs] = createSignal([1, 2]);
export function App() {
  const track = createReaction(() => { log("reaction", own()); const v = w(); log("reaction", "read " + v); });
  track(() => c());
  return <i>r</i>;
}
export async function after() {
  setC(2); await new Promise((r) => setTimeout(r, 20));
}