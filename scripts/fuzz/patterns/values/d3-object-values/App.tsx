import { createSignal, createMemo, createStore, untrack, merge, omit, snapshot, createContext, useContext, createEffect, flush, For } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
export function App() {
  const [a] = createSignal(1); const accessors = { a }; let t = 0; for (const f of Object.values(accessors)) t += f(); log(t);
  return <div><p>{a()}</p></div>;
}
