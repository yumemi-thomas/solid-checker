import { createSignal, createMemo, createOptimistic, children, mapArray, repeat, latest, isPending, refresh, getOwner, For, flush } from "solid-js";
import type { JSX } from "solid-js";
declare const __log: (...a: any[]) => void;
const [c, setC] = createSignal(1);
const [w, setW] = createSignal(0);
const [list, setList] = createSignal([1, 2]);
const own = () => (getOwner() as any)?._name ?? String(getOwner());
export function App() {
  const v = latest(() => c());
  return <p>{v}</p>;
}
