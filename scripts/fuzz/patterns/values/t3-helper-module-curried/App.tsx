import { createSignal, createMemo, createStore, untrack, merge, omit, snapshot, createContext, useContext, createEffect, flush, For } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
const dbl = (g: () => number) => () => g() * 2;
export function App() {
  const [a] = createSignal(1); const d = dbl(a); const x = d(); log(x);
  return <div><p>{a()}</p></div>;
}
