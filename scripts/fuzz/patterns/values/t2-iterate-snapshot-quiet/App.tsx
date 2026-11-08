import { createSignal, createMemo, createStore, untrack, merge, omit, snapshot, createContext, useContext, createEffect, flush, For } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
export function App() {
  const [st, setSt] = createStore({ items: [{ id: 1, v: 1 }] });
  return <div><button onClick={() => { snapshot(st).items.forEach((x) => { x.v = 5; }); }}>go</button></div>;
}
