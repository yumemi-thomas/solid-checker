import { createSignal, createMemo, createOptimistic, children, mapArray, repeat, latest, isPending, refresh, getOwner, For, flush } from "solid-js";
import type { JSX } from "solid-js";
declare const __log: (...a: any[]) => void;
const [c, setC] = createSignal(1);
const [w, setW] = createSignal(0);
const [list, setList] = createSignal([1, 2]);
const own = () => (getOwner() as any)?._name ?? String(getOwner());
export function App() {
  return <ul><For each={list()} keyed={(x) => { __log("key owner", own()); c(); return x; }}>{(item) => <li>{item()}</li>}</For></ul>;
}
export async function after() { setC(2); }
