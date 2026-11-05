import { createSignal, createMemo, createStore, untrack, merge, omit, snapshot, createContext, useContext, createEffect, flush, For } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
export function App() {
  const [st, setSt] = createStore({ n: 1 }); const bump = (o: { n: number }) => { o.n = 5; };
  return <div><button onClick={() => { setSt(() => { bump(st); }); flush(); log("snap=" + untrack(() => JSON.stringify(st))); }}>go</button></div>;
}
