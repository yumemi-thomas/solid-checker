import { createSignal, createMemo, createStore, untrack, merge, omit, snapshot, createContext, useContext, createEffect, flush, For } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
export function App() {
  const [st, setSt] = createStore([1, 2]);
  return <div><button onClick={() => { st.fill(0, 1, 1); }}>go</button></div>;
}
