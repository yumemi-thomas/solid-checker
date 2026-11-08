import { createSignal, createMemo, createStore, untrack, merge, omit, snapshot, createContext, useContext, createEffect, flush, For } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
export function App() {
  const [a] = createSignal(1); (window as any).acc = a; const x = (window as any).acc(); log(x);
  return <div><p>{a()}</p></div>;
}
