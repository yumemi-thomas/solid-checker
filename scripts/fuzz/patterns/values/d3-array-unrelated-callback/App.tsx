import { createSignal, createMemo, createStore, untrack, merge, omit, snapshot, createContext, useContext, createEffect, flush, For } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
export function App() {
  const [a] = createSignal(1); const fs = [a]; const n = fs.length; const rows = [1, 2].map((f) => f + n); log(rows.join());
  return <div><p>{a()}</p></div>;
}
