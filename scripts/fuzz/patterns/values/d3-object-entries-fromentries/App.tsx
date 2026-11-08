import { createSignal, createMemo, createStore, untrack, merge, omit, snapshot, createContext, useContext, createEffect, flush, For } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
export function App() {
  const [a] = createSignal(1); const acc = { a }; const v = Object.fromEntries(Object.entries(acc).map(([k, f]) => [k, f()])); log(JSON.stringify(v));
  return <div><p>{a()}</p></div>;
}
