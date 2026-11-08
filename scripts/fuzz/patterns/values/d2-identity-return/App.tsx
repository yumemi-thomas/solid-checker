import { createSignal, createMemo, createStore, untrack, merge, omit, snapshot, createContext, useContext, createEffect, flush, For } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
export function App() {
  const [st, setSt] = createStore({ n: 1 }); const id = <T,>(v: T): T => v;
  return <div><button onClick={() => { id(st).n = 5; }}>go</button></div>;
}
