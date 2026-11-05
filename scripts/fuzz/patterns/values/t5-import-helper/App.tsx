import { pick } from "./h";
import { createSignal, createMemo, createStore, untrack, merge, omit, snapshot, createContext, useContext, createEffect, flush, For } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
function Child(props: { a: number }) { const x = pick(props); log(x); return <i>x</i>; }
export function App() {
  const [s] = createSignal(1);
  return <div><Child a={s()} /></div>;
}
