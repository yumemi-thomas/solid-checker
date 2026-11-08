import { createSignal, createMemo, createStore, untrack, merge, omit, snapshot, createContext, useContext, createEffect, flush, For } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
export function App() {
  const [st, setSt] = createStore({}); const x = String(st); log(x);
  return <div><p>x</p></div>;
}
