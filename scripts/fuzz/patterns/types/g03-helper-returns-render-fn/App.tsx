import { createSignal, createMemo, createOptimistic, children, mapArray, repeat, latest, isPending, refresh, getOwner, For, flush } from "solid-js";
import type { JSX } from "solid-js";
declare const __log: (...a: any[]) => void;
const [c, setC] = createSignal(1);
const [w, setW] = createSignal(0);
const [list, setList] = createSignal([1, 2]);
const own = () => (getOwner() as any)?._name ?? String(getOwner());
function part() { return () => { const v = c(); __log("part fn owner", own(), v); return <p>{v}</p>; }; }
export function App() {
  return <div>{part()}</div>;
}
export async function after() { setC(2); }
