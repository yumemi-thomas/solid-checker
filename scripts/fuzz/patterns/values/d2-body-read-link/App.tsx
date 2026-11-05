import { createSignal, createMemo, createStore, untrack, merge, omit, snapshot, createContext, useContext, createEffect, flush, For } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
export function App() {
  const [st, setSt] = createStore({ user: { n: 1 } }); const fmt = (u: { n: number }) => "n" + u.n; const x = fmt(st.user); log(x);
  return <div><p>x</p></div>;
}
