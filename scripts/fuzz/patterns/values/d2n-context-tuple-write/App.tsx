import { createSignal, createMemo, createStore, untrack, merge, omit, snapshot, createContext, useContext, createEffect, flush, For } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
const Ctx = createContext<[{ n: number }, any]>();
function Child() { const [st] = useContext(Ctx)!; return <button onClick={() => { st.n = 5; }}>go</button>; }
export function App() {
  const pair = createStore({ n: 1 });
  return <div><Ctx value={pair}><Child /></Ctx></div>;
}
