import { createSignal, createMemo, createStore, untrack, merge, omit, snapshot, createContext, useContext, createEffect, flush, For } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
export function App() {
  const [st, setSt] = createStore({ a: 1 }); const o = Object.create(st); o.a = 2; log("skip");
  return <div><p>x</p></div>;
}
