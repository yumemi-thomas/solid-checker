import { createSignal, createMemo, createStore, untrack, merge, omit, snapshot, createContext, useContext, createEffect, flush, For } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
const Ctx = createContext<{ st: { n: number } }>();
function Child() { const c = useContext(Ctx)!; return <button onClick={() => { c.st.n = 5; }}>go</button>; }
export function App() {
  const [st, setSt] = createStore({ n: 1 });
  return <div><Ctx value={{ st }}><Child /></Ctx></div>;
}
