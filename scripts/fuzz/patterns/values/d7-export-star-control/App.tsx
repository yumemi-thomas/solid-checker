import { createSignal as cs } from "./re";
import { createSignal, createMemo, createStore, untrack, merge, omit, snapshot, createContext, useContext, createEffect, flush, For } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
export function App() {
  const [a] = cs(1); const x = a(); log(x);
  return <div><p>{a()}</p></div>;
}
