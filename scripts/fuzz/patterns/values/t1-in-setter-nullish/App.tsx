import { createSignal, createMemo, createStore, untrack, merge, omit, snapshot, createContext, useContext, createEffect, flush, For } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
export function App() {
  const [st, setSt] = createStore<{ a: number | null }>({ a: null });
  return <div><button onClick={() => { setSt(() => { st.a ??= 5; }); flush(); log(untrack(() => JSON.stringify(st))); }}>go</button></div>;
}
