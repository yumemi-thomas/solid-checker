import { createSignal, createMemo, createStore, untrack, merge, omit, snapshot, createContext, useContext, createEffect, flush, For } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
export function App() {
  const [a] = createSignal(1); const dbl = (g: () => number) => () => g() * 2; const d = dbl(a);
  return <div><p>{d()}</p></div>;
}
