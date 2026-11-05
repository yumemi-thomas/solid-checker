import { createSignal, createMemo, createStore, untrack, merge, omit, snapshot, createContext, useContext, createEffect, flush, For } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
function Child(props: { st: { n: number } }) { return <button onClick={() => { props.st.n = 5; }}>go</button>; }
export function App() {
  const [st, setSt] = createStore({ n: 1 });
  return <div><Child st={st} /></div>;
}
