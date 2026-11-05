import { Solid } from "./re";
import { createSignal, createMemo, createStore, untrack, merge, omit, snapshot, createContext, useContext, createEffect, flush, For } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
export function App() {
  const [st] = Solid.createStore({ a: 1 });
  return <div><button onClick={() => { st.a = 5; }}>go</button></div>;
}
