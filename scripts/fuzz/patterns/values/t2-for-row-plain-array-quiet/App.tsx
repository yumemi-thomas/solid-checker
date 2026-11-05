import { createSignal, createMemo, createStore, untrack, merge, omit, snapshot, createContext, useContext, createEffect, flush, For } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
export function App() {
  const rows = [{ v: 1 }];
  return <div><For each={rows}>{(x) => <button onClick={() => { x.v = 2; }}>go</button>}</For></div>;
}
