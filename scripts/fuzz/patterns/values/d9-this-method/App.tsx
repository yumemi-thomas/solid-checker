import { createSignal, createMemo, createStore, untrack, merge, omit, snapshot, createContext, useContext, createEffect, flush, For } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
export function App() {
  const [s] = createSignal(1); const o = { s, v() { return this.s(); } }; const x = o.v(); log(x);
  return <div><p>{s()}</p></div>;
}
