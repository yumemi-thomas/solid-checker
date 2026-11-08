import { createSignal, createMemo, createStore, untrack, merge, omit, snapshot, createContext, useContext, createEffect, flush, For } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
export function App() {
  const [st, setSt] = createStore({ n: 1 }); const bump = (o: { n: number }) => { o.n = 5; }; const plain = { n: 0 }; bump(plain);
  return <div><button onClick={() => { setSt((d) => { bump(d); }); }}>go</button></div>;
}
