import { createSignal, createMemo, createStore, untrack, merge, omit, snapshot, createContext, useContext, createEffect, flush, For } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
export function App() {
  const [a] = createSignal(1); const [b] = createSignal(2); const sources = [a, b]; let t = 0; for (const f of sources) t += f(); log(t);
  return <div><p>{a()}</p></div>;
}
