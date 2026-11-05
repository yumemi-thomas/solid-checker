import { createSignal, createMemo, createStore, untrack, merge, omit, snapshot, createContext, useContext, createEffect, flush, For } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
function wrap(o: { a: number }) { return () => o.a; }
function Child(props: { a: number; b?: number }) { const g = wrap(props); const x = g(); log(x); return <i>x</i>; }
export function App() {
  const [s] = createSignal(1);
  return <div><Child a={s()} /></div>;
}
