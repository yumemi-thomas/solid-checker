import { createSignal, createMemo, createStore, untrack, merge, omit, snapshot, createContext, useContext, createEffect, flush, For } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
export function App() {
  const [st, setSt] = createStore({ list: [1, 2] });
  return <div><button onClick={() => { let t = 0; st.list.forEach((x) => { t += x; }); log(t); }}>go</button></div>;
}
