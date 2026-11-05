import { Solid } from "./re";
import { createSignal, createMemo, createStore, untrack, merge, omit, snapshot, createContext, useContext, createEffect, flush, For } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
export function App() {
  const [a] = Solid.createSignal(1); const x = a(); log(x);
  return <div><p>{a()}</p></div>;
}
