import { createSignal, createMemo, createStore, untrack, merge, omit, snapshot, createContext, useContext, createEffect, flush, For } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
export function App() {
  const [st, setSt] = createStore({ a: 1 });
  return <div><button onClick={() => { Reflect.defineProperty(st, "a", { value: 5 }); }}>go</button></div>;
}
