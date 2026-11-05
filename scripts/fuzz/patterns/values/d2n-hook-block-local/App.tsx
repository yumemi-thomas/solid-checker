import { createSignal, createMemo, createStore, untrack, merge, omit, snapshot, createContext, useContext, createEffect, flush, For } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
function useThing() { const [state, setState] = createStore({ n: 1 }); return [state, setState] as const; }
export function App() {
  const [state] = useThing();
  return <div><button onClick={() => { state.n = 5; }}>go</button></div>;
}
