import { createSignal, createMemo, createStore, untrack, merge, omit, snapshot, createContext, useContext, createEffect, flush, For } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
export function App() {
  const [a] = createSignal(1); const [b] = createSignal(2); const total = [a, b].map((f) => f()).reduce((x, y) => x + y, 0); log(total);
  return <div><p>{a()}</p></div>;
}
