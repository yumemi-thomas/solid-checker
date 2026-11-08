import { createSignal, createMemo, createStore, untrack, merge, omit, snapshot, createContext, useContext, createEffect, flush, For } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
export function App() {
  const [a] = createSignal(1); const fs = [a]; createEffect(() => fs.map((f) => f()), (v) => { log(v); });
  return <div><p>x</p></div>;
}
