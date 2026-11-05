import { createSignal, createMemo, createStore, untrack, merge, omit, snapshot, createContext, useContext, createEffect, flush, For } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
function Child(props: { a: number; b?: number }) { const x = Object.values(props); log(x.length); return <i>x</i>; }
export function App() {
  const [s] = createSignal(1);
  return <div><Child a={s()} /></div>;
}
